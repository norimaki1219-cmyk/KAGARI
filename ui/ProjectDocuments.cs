using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Text.RegularExpressions;
using System.Windows.Documents;
using System.Windows.Automation;
using System.Xml.Linq;

internal sealed partial class Desktop {
 static readonly StringComparer DocComparer=StringComparer.OrdinalIgnoreCase;
 string contextFile="",contextRoot="";
 XElement[] contextTasks=new XElement[0];
 CancellationTokenSource relatedScan;
 static string EnsureMainDocument(string root){
  foreach(string sub in new[]{"","docs","design","設計","app/docs"}){
   foreach(string name in new[]{"システム構成.md","ARCHITECTURE.md","システム構成図.md","アーキテクチャ設計.md","設計_システム構成の再検討.md"}){
    string candidate=Path.Combine(root,sub,name);
    if(File.Exists(candidate))return candidate;
   }
  }
  var directory=new DirectoryInfo(root);
  var ignored=new HashSet<string>("target node_modules Library Temp Logs obj bin build dist __pycache__".Split(' '),DocComparer);
  var entries=directory.GetFileSystemInfos().Where(f=>!f.Name.StartsWith(".") && (f.Attributes&(FileAttributes.Hidden|FileAttributes.ReparsePoint))==0 && !ignored.Contains(f.Name)).OrderBy(f=>f.Name,DocComparer).ToList();
  var body=new StringBuilder().AppendLine("# "+directory.Name+" のシステム構成").AppendLine();
  var dirs=entries.OfType<DirectoryInfo>().ToList();
  body.AppendLine("## フォルダー構成").AppendLine();
  foreach(var d in dirs)body.AppendLine("- `"+d.Name+"/`");
  if(dirs.Count==0)body.AppendLine("直下のフォルダーはありません。");
  string[,] components={{"Cargo.toml","Rust パッケージ"},{"package.json","Node.js パッケージ"},{"pyproject.toml","Python パッケージ"},{"requirements.txt","Python 依存パッケージ"},{"wrangler.toml","Cloudflare 配備設定"},{"wrangler.jsonc","Cloudflare 配備設定"},{"Dockerfile","コンテナ定義"},{"compose.yml","コンテナ構成"},{"ProjectSettings/ProjectVersion.txt","Unity プロジェクト設定"},{"go.mod","Go モジュール"}};
  var definitions=new List<string>();
  foreach(string parent in new[]{""}.Concat(dirs.Select(d=>d.Name))){
   for(int i=0;i<components.GetLength(0);i++){
    string name=Path.Combine(parent,components[i,0]);
    if(File.Exists(Path.Combine(root,name)))definitions.Add("- "+components[i,1]+"　`"+name.Replace('\\','/')+"`");
   }
  }
  if(definitions.Count>0){body.AppendLine().AppendLine("## 実装・実行環境").AppendLine();foreach(string line in definitions)body.AppendLine(line);}
  var docs=entries.OfType<FileInfo>().Where(f=>IsMarkdownFile(f.Name)).ToList();
  if(docs.Count>0){
   body.AppendLine().AppendLine("## 関連資料").AppendLine();
   foreach(var f in docs)body.AppendLine("- ["+f.Name.Replace("]","\\]")+"]("+Uri.EscapeDataString(f.Name)+")");
  }
  string path=Path.Combine(root,"システム構成.md"),temp=Path.Combine(root,".kagari-"+Guid.NewGuid().ToString("N")+".tmp");
  try{File.WriteAllText(temp,body.ToString(),new UTF8Encoding(false));try{File.Move(temp,path);}catch(IOException){if(!File.Exists(path))throw;}}
  finally{if(File.Exists(temp))File.Delete(temp);}
  return path;
 }
 static bool IsMarkdownFile(string path){return DocComparer.Equals(Path.GetExtension(path),".md") || DocComparer.Equals(Path.GetExtension(path),".markdown");}
 string[] DocumentRoots(){return data.Elements("project").Select(p=>PathName(p.Value)).ToArray();}
 CancellationTokenSource taskDocumentSearch;
 void CancelTaskDocumentSearch(){
  if(taskDocumentSearch!=null)taskDocumentSearch.Cancel();
 }
 sealed class TaskDocMatch {public string Path;public int Score;}
 static Regex TaskTicket(string title){
  var id=Regex.Match(title,@"^[A-Za-z]{1,12}-?\d+(?![A-Za-z0-9])");
  return id.Success?new Regex(@"(?<![A-Za-z0-9])"+Regex.Escape(id.Value)+@"(?![A-Za-z0-9])",RegexOptions.IgnoreCase|RegexOptions.CultureInvariant):null;
 }
 static TaskDocMatch FindTaskSection(string root,string title,string[] attached,CancellationToken cancel){
  var candidates=new HashSet<string>(DocComparer);var folders=new Queue<string>();
  folders.Enqueue(root);
  foreach(var file in attached)if(File.Exists(file))candidates.Add(file);
  while(folders.Count>0){
   cancel.ThrowIfCancellationRequested();string folder=folders.Dequeue();
   try{
    foreach(var entry in new DirectoryInfo(folder).EnumerateFileSystemInfos()){
     cancel.ThrowIfCancellationRequested();
     if((entry.Attributes&(FileAttributes.ReparsePoint|FileAttributes.Hidden))!=0)continue;
     if(entry is FileInfo){if(IsMarkdownFile(entry.Name))candidates.Add(entry.FullName);}
     else if(folder!=root || new[]{"doc","docs","documents","spec","仕様","資料"}.Contains(entry.Name,DocComparer))folders.Enqueue(entry.FullName);
    }
   }catch(IOException){}catch(UnauthorizedAccessException){}
  }
  var ticket=TaskTicket(title);TaskDocMatch best=null;
  foreach(string file in candidates.OrderBy(p=>p,DocComparer)){
   cancel.ThrowIfCancellationRequested();int score=0;
   try{
    if(new FileInfo(file).Length>512*1024)continue;
    string stem=Path.GetFileNameWithoutExtension(file);
    if(DocComparer.Equals(stem,title))score=100;
    else if(ticket!=null && ticket.IsMatch(stem))score=85;
    else if(ticket!=null && file.Substring(root.TrimEnd('\\').Length+1).Split('\\').Any(part=>ticket.IsMatch(part)))score=65;
    foreach(var line in File.ReadLines(file)){
     cancel.ThrowIfCancellationRequested();string heading=line.TrimStart();
     bool isHeading=heading.StartsWith("#");bool exact=line.IndexOf(title,StringComparison.OrdinalIgnoreCase)>=0;
     int rank=exact?(isHeading?95:75):ticket!=null && ticket.IsMatch(line)?(isHeading?80:55):0;
     score=Math.Max(score,rank);
    }
    if(score>0 && (best==null || score>best.Score))best=new TaskDocMatch{Path=file,Score=score};
   }catch(IOException){}catch(UnauthorizedAccessException){}
  }
  return best;
 }
 void FocusTaskSection(string title){
  var box=At<RichTextBox>("Preview");var ticket=TaskTicket(title);TextPointer start=null,end=null;int best=0;
  for(var pos=box.Document.ContentStart;pos!=null;pos=pos.GetNextContextPosition(LogicalDirection.Forward)){
   if(pos.GetPointerContext(LogicalDirection.Forward)!=TextPointerContext.Text)continue;
   string text=pos.GetTextInRun(LogicalDirection.Forward);
   int index=text.IndexOf(title,StringComparison.OrdinalIgnoreCase),length=title.Length,rank=2;
   if(index<0){if(ticket==null)continue;var m=ticket.Match(text);if(!m.Success)continue;index=m.Index;length=m.Length;rank=1;}
   if(rank<=best)continue;best=rank;start=pos.GetPositionAtOffset(index);end=start.GetPositionAtOffset(length);if(rank==2)break;
  }
  if(start!=null && start.Paragraph!=null){
   new TextRange(start,end).ApplyPropertyValue(TextElement.BackgroundProperty,SystemParameters.HighContrast?SystemColors.HighlightBrush:Color("#FFE39A"));
   if(SystemParameters.HighContrast)new TextRange(start,end).ApplyPropertyValue(TextElement.ForegroundProperty,SystemColors.HighlightTextBrush);
   box.UpdateLayout();start.Paragraph.BringIntoView();
  }else box.ScrollToHome();
 }
 async void NavigateTaskDocument(){
  var row=data.Elements("task").FirstOrDefault(t=>Value(t,"id")==Value(data,"task"));
  if(row==null)return;
  string key=Value(row,"key"),title=Value(row,"title").Trim(),root=DocumentProject();
  if(title.Length==0 || !Directory.Exists(root))return;
  string current=Value(data.Element("preview"),"path");
  if(current.Length>0 && File.Exists(FullTaskDoc(root,current))){FocusTaskSection(title);return;}
  var task=ReadDocumentTasks(root).Elements("task").FirstOrDefault(t=>Value(t,"key")==key);
  var registered=task==null?null:task.Elements("doc").FirstOrDefault(d=>File.Exists(FullTaskDoc(root,d.Value)));
  if(registered!=null){OpenProjectFile(root,FullTaskDoc(root,registered.Value));FocusTaskSection(title);return;}
  var source=new CancellationTokenSource();taskDocumentSearch=source;
  try{
   var prefix=root.TrimEnd('\\')+"\\";
   var files=ReadDocumentTasks(root).Descendants("doc").Select(d=>FullTaskDoc(root,d.Value)).Where(f=>f.StartsWith(prefix,StringComparison.OrdinalIgnoreCase)).Distinct(DocComparer).ToArray();
   var match=await Task.Run(()=>FindTaskSection(root,title,files,source.Token),source.Token);
   source.Token.ThrowIfCancellationRequested();
   if(!data.Elements("task").Any(t=>Value(t,"id")==Value(data,"task") && Value(t,"key")==key))return;
   string path=match!=null?match.Path:EnsureMainDocument(root);
   if(match==null)projectHomeFile=path;
   OpenProjectFile(root,path);FocusTaskSection(title);
   var note=At<TextBlock>("CurrentRelation");note.Background=Brushes.Transparent;note.Foreground=SystemColors.ControlTextBrush;
   note.Text=match!=null?"「"+title+"」に一致する資料":"「"+title+"」の該当資料が見つからないため、システム構成を表示しています。";note.Visibility=Visibility.Visible;
  }catch(OperationCanceledException){}catch(IOException ex){At<TextBlock>("FileName").Text=ex.Message;}catch(UnauthorizedAccessException ex){At<TextBlock>("FileName").Text=ex.Message;}
  finally{if(taskDocumentSearch==source)taskDocumentSearch=null;source.Dispose();}
 }
 string projectHomeFile;
 void SetDocumentTab(Button selected){
  foreach(Button tab in At<WrapPanel>("Tabs").Children){
   bool active=tab==selected;tab.Background=active?(SystemParameters.HighContrast?SystemColors.HighlightBrush:Color("#175E62")):Brushes.Transparent;
   tab.Foreground=active?(SystemParameters.HighContrast?SystemColors.HighlightTextBrush:Brushes.White):SystemColors.ControlTextBrush;
   tab.FontWeight=active?FontWeights.Bold:FontWeights.Normal;
   System.Windows.Automation.AutomationProperties.SetItemStatus(tab,active?"選択中":"");
  }
 }
 void OpenProjectHome(){
  var roots=DocumentRoots();int index=(int)data.Attribute("project");
  if(index<0 || index>=roots.Length)return;
  try{projectHomeFile=EnsureMainDocument(roots[index]);OpenProjectFile(roots[index],projectHomeFile);}
  catch(Exception ex){At<TextBlock>("FileName").Text="システム構成を開けません。"+ex.Message;}
 }
 void OpenProjectFile(string root,string path){
  string prefix=root.TrimEnd('\\')+"\\";
  if(!path.StartsWith(prefix,StringComparison.OrdinalIgnoreCase))return;
  string relative=path.Substring(prefix.Length);
  if(DocComparer.Equals(root,DocumentProject()))Run(704,0,relative);
  else{
   int index=Array.FindIndex(DocumentRoots(),p=>DocComparer.Equals(p,root));
   if(index>=0){explorerPath=path;Run(717,index,relative);}
  }
 }
 [DllImport("kagari_core")]
 static extern IntPtr kagari_checklist_rtf(IntPtr h,byte[] source,UIntPtr size,uint token,out int length);

 void BindDocumentChecks(RichTextBox box,string relative){
  string path=explorerPath??FullTaskDoc(DocumentProject(),relative);
  if(!IsMarkdownFile(path) || !File.Exists(path))return;
  try{
   if(new FileInfo(path).Length>512*1024)return;
   byte[] snapshot=File.ReadAllBytes(path);
   string source=new UTF8Encoding(false,true).GetString(snapshot);
   if(!Regex.IsMatch(source,@"(?m)^[^\S\r\n\uFEFF]*[\uFEFF]?[^\S\r\n]*[-*+] \[[ xX]\] "))return;
   uint token;string marker;
   do{token=BitConverter.ToUInt32(Guid.NewGuid().ToByteArray(),0);marker="KagariCheck"+token.ToString(System.Globalization.CultureInfo.InvariantCulture)+"Z";}while(source.Contains(marker));
   int length;
   IntPtr result=kagari_checklist_rtf(core,snapshot,(UIntPtr)(uint)snapshot.Length,token,out length);
   if(result==IntPtr.Zero)return;
   byte[] rtf=new byte[length];Marshal.Copy(result,rtf,0,length);
   var document=new FlowDocument();
   using(var stream=new MemoryStream(rtf))new TextRange(document.ContentStart,document.ContentEnd).Load(stream,DataFormats.Rtf);
   var pattern=new Regex(Regex.Escape(marker)+@"(\d+):");
   for(var pos=document.ContentStart;pos!=null;pos=pos.GetNextContextPosition(LogicalDirection.Forward)){
    if(pos.GetPointerContext(LogicalDirection.Forward)!=TextPointerContext.Text)continue;
    var match=pattern.Match(pos.GetTextInRun(LogicalDirection.Forward));if(!match.Success)continue;
    int offset=int.Parse(match.Groups[1].Value,System.Globalization.CultureInfo.InvariantCulture);
    var start=pos.GetPositionAtOffset(match.Index);
    new TextRange(start,start.GetPositionAtOffset(match.Length)).Text="";
    var check=new CheckBox{IsChecked=snapshot[offset]!=(byte)' ',Margin=new Thickness(0,0,4,0),ToolTip="クリックして資料に保存"};
    AutomationProperties.SetName(check,new TextRange(start,start.Paragraph.ContentEnd).Text.Trim());
    new InlineUIContainer(check,start){BaselineAlignment=BaselineAlignment.Center};
    check.Click+=(sender,args)=>{
     try{SaveDocumentCheck(path,snapshot,offset,check.IsChecked==true);}
     catch(Exception ex){check.IsChecked=snapshot[offset]!=(byte)' ';MessageBox.Show(window,ex.Message,"チェックを保存できません");}
    };
   }
   box.Document=document;
  }catch(IOException){}catch(UnauthorizedAccessException){}catch(DecoderFallbackException){}
 }

 static void SaveDocumentCheck(string path,byte[] snapshot,int offset,bool value){
  using(var stream=new FileStream(path,FileMode.Open,FileAccess.ReadWrite,FileShare.None)){
   if(stream.Length!=snapshot.Length)throw new IOException("資料が更新されています。資料を開き直してください。");
   byte[] current=new byte[snapshot.Length];
   using(var reader=new BinaryReader(stream,Encoding.UTF8,true)){
    int count=0;
    while(count<current.Length){
     int n=reader.Read(current,count,current.Length-count);
     if(n==0)throw new EndOfStreamException();
     count+=n;
    }
   }
   if(!current.SequenceEqual(snapshot))throw new IOException("資料が更新されています。資料を開き直してください。");
   byte next=value?(byte)'x':(byte)' ';
   stream.Position=offset;stream.WriteByte(next);stream.Flush(true);snapshot[offset]=next;
  }
 }

 void WireProjectDocuments(){
  At<Button>("ProjectHome").Click+=(s,e)=>OpenProjectHome();
  At<Button>("LinkedTasks").Click+=(s,e)=>{
   var menu=new ContextMenu();
   foreach(var task in contextTasks){
    string key=Value(task,"key");
    AddDocumentItem(menu,Value(task,"title"),()=>Run(746,0,key));
   }
   ShowDocumentMenu("LinkedTasks",menu);
  };
  At<Button>("LinkedDocs").Click+=(s,e)=>ShowRelatedDocuments();
  At<Button>("DocHistory").Click+=(s,e)=>ShowDocumentHistory();
 }
 static void AddDocumentItem(ContextMenu menu,string label,Action action){
  var item=new MenuItem{Header=new TextBlock{Text=label},IsEnabled=action!=null};
  if(action!=null)item.Click+=(s,e)=>action();
  menu.Items.Add(item);
 }
 void ShowDocumentMenu(string name,ContextMenu menu){
  if(menu.Items.Count==0)AddDocumentItem(menu,"該当する項目はありません",null);
  menu.PlacementTarget=At<Button>(name);menu.IsOpen=true;
 }
 static string FullTaskDoc(string root,string relative){
  try{return Path.GetFullPath(Path.Combine(root,relative));}catch(ArgumentException){return "";}catch(NotSupportedException){return "";}catch(IOException){return "";}
 }
 XElement ReadDocumentTasks(string root){
  int index=Array.FindIndex(DocumentRoots(),p=>DocComparer.Equals(p,root));
  return index<0?new XElement("tasks"):XElement.Parse(Marshal.PtrToStringUni(kagari_action(core,745,index,"")));
 }
 void RefreshDocumentContext(){
  string root=DocumentProject(),path=Value(data.Element("preview"),"path");
  if(explorerPath!=null){
   path=explorerPath;
   root=DocumentRoots().Where(p=>path.StartsWith(p.TrimEnd('\\')+"\\",StringComparison.OrdinalIgnoreCase)).OrderByDescending(p=>p.Length).FirstOrDefault()??"";
  }
  path=path.Length==0?"":FullTaskDoc(root,path);
  bool show=root.Length>0 && IsMarkdownFile(path) && File.Exists(path) && At<ScrollViewer>("ExplorerPreview").Visibility!=Visibility.Visible;
  At<StackPanel>("DocumentContext").Visibility=Visible(show);
  if(!DocComparer.Equals(contextFile,path) && relatedScan!=null)relatedScan.Cancel();
  contextFile=path;contextRoot=root;
  At<TextBlock>("CurrentRelation").Visibility=Visibility.Collapsed;
  var tabs=At<WrapPanel>("Tabs");
  Button activeTab=DocComparer.Equals(projectHomeFile,path)?At<Button>("ProjectHome"):null;
  SetDocumentTab(activeTab);
  if(!show)return;
  var tasks=ReadDocumentTasks(root).Elements("task");
  contextTasks=tasks.Where(t=>t.Elements("doc").Any(d=>DocComparer.Equals(FullTaskDoc(root,d.Value),path))).ToArray();
  var selected=data.Elements("task").FirstOrDefault(t=>Value(t,"id")==Value(data,"task"));
  var current=contextTasks.FirstOrDefault(t=>selected!=null && Value(t,"key")==Value(selected,"key"));
  var badge=At<TextBlock>("CurrentRelation");
  badge.Text=current==null?(contextTasks.Length==0?"関連付けられたタスクはありません":"関連タスク　"+Value(contextTasks[0],"title")+(contextTasks.Length>1?"　ほか "+(contextTasks.Length-1)+" 件":"")):
   "選択中のタスク　"+Value(current,"title")+"　／　"+String.Join("・",current.Elements("doc").Where(d=>DocComparer.Equals(FullTaskDoc(root,d.Value),path)).Select(d=>Value(d,"kind")));
  if(activeTab==null && current!=null){
   var kinds=current.Elements("doc").Where(d=>DocComparer.Equals(FullTaskDoc(root,d.Value),path)).Select(d=>Value(d,"kind")).ToArray();
   var matches=tabs.Children.OfType<Button>().Where(b=>b.Tag!=null && kinds.Contains(b.Content as string)).ToArray();
   SetDocumentTab(matches.FirstOrDefault(b=>Convert.ToInt32(b.Tag)==175+(int)data.Attribute("doc"))??matches.FirstOrDefault());
  }
  badge.Visibility=Visible(contextTasks.Length>0);
  badge.Background=current==null?Brushes.Transparent:SystemParameters.HighContrast?SystemColors.HighlightBrush:Color("#E2F1EE");
  badge.Foreground=current==null?SystemColors.ControlTextBrush:SystemParameters.HighContrast?SystemColors.HighlightTextBrush:Color("#154D4C");
  At<Button>("LinkedTasks").Content="関連タスク ("+contextTasks.Length+")";
  try{CaptureDocument(path);}catch(IOException ex){badge.Visibility=Visibility.Visible;badge.Text+="　履歴保存エラー　"+ex.Message;}catch(UnauthorizedAccessException ex){badge.Visibility=Visibility.Visible;badge.Text+="　履歴保存エラー　"+ex.Message;}
 }
 async void ShowRelatedDocuments(){
  string root=contextRoot,file=contextFile;
  if(relatedScan!=null)relatedScan.Cancel();
  var source=new CancellationTokenSource();relatedScan=source;
  var menu=new ContextMenu();
  var seen=new HashSet<string>(DocComparer);
  Action<string,string> add=(path,reason)=>{
   if(!DocComparer.Equals(file,path) && path.StartsWith(root.TrimEnd('\\')+"\\",StringComparison.OrdinalIgnoreCase) && File.Exists(path) && seen.Add(path))
    AddDocumentItem(menu,reason+"　"+path.Substring(root.TrimEnd('\\').Length+1),()=>OpenProjectFile(root,path));
  };
  foreach(var task in contextTasks)foreach(var doc in task.Elements("doc"))add(FullTaskDoc(root,doc.Value),"同じタスク");
  var loading=new MenuItem{Header="リンクを確認中…",IsEnabled=false};menu.Items.Add(loading);
  menu.Closed+=(s,e)=>{if(relatedScan==source)source.Cancel();};ShowDocumentMenu("LinkedDocs",menu);
  try{
   var otherProjects=new HashSet<string>(DocumentRoots().Where(p=>!DocComparer.Equals(p,root)),DocComparer);
   var map=await Task.Run(()=>ProjectMap.Read(root,source.Token,otherProjects),source.Token);
   source.Token.ThrowIfCancellationRequested();
   MapNode node;
   if(map.Nodes.TryGetValue(file.Substring(root.TrimEnd('\\').Length+1),out node)){
    foreach(var edge in node.To.Where(p=>p.Key.Kind=="Markdown"))add(Path.Combine(root,edge.Key.Path),"参照先");
    foreach(var edge in node.From.Where(p=>p.Key.Kind=="Markdown"))add(Path.Combine(root,edge.Key.Path),"参照元");
   }
   if(map.Skipped>0)AddDocumentItem(menu,"一部のファイルは読み取れませんでした",null);
  }catch(OperationCanceledException){}catch(Exception ex){AddDocumentItem(menu,ex.Message,null);}
  finally{menu.Items.Remove(loading);if(menu.Items.Count==0)AddDocumentItem(menu,"関連資料はありません",null);if(relatedScan==source)relatedScan=null;source.Dispose();}
 }
 static string Digest(byte[] bytes){using(var hash=SHA256.Create())return BitConverter.ToString(hash.ComputeHash(bytes)).Replace("-","");}
 static string VersionFolder(string path){return Path.Combine(Environment.GetEnvironmentVariable("LOCALAPPDATA")??Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"RustAIWorkspace","document-history",Digest(Encoding.UTF8.GetBytes(Path.GetFullPath(path).ToUpperInvariant())).Substring(0,32));}
 static void CaptureDocument(string path){
  var file=new FileInfo(path);if(file.Length>512*1024)return;
  byte[] bytes=File.ReadAllBytes(path);
  string directory=VersionFolder(path),suffix="-"+Digest(bytes)+".md";
  Directory.CreateDirectory(directory);
  string last=Directory.EnumerateFiles(directory,"*.md").OrderByDescending(p=>p,StringComparer.Ordinal).FirstOrDefault();
  if(last!=null && last.EndsWith(suffix,StringComparison.Ordinal))return;
  string version=Path.Combine(directory,DateTime.UtcNow.ToString("yyyyMMddHHmmssfffffff",System.Globalization.CultureInfo.InvariantCulture)+suffix);
  string temp=Path.Combine(directory,Guid.NewGuid().ToString("N")+".tmp");
  try{File.WriteAllBytes(temp,bytes);File.Move(temp,version);}finally{if(File.Exists(temp))File.Delete(temp);}
 }
 void ShowDocumentHistory(){
  string file=contextFile;
  var menu=new ContextMenu();
  try{
   CaptureDocument(file);
   foreach(var version in Directory.GetFiles(VersionFolder(file),"*.md").OrderByDescending(p=>p,StringComparer.Ordinal)){
    string snapshot=version,stamp=Path.GetFileName(version).Split('-')[0];
    DateTime date=DateTime.ParseExact(stamp,"yyyyMMddHHmmssfffffff",System.Globalization.CultureInfo.InvariantCulture,System.Globalization.DateTimeStyles.AssumeUniversal|System.Globalization.DateTimeStyles.AdjustToUniversal);
    string label=date.ToLocalTime().ToString("yyyy/MM/dd HH:mm:ss");
    AddDocumentItem(menu,label,()=>{
     try{new Window{Owner=window,Title=Path.GetFileName(file)+"　履歴　"+label,Width=760,Height=540,WindowStartupLocation=WindowStartupLocation.CenterOwner,Content=new TextBox{Text=File.ReadAllText(snapshot),IsReadOnly=true,TextWrapping=TextWrapping.Wrap,VerticalScrollBarVisibility=ScrollBarVisibility.Auto,Padding=new Thickness(12)}}.Show();}
     catch(IOException ex){MessageBox.Show(window,ex.Message,"変更履歴");}
    });
   }
  }catch(Exception ex){AddDocumentItem(menu,ex.Message,null);}
  ShowDocumentMenu("DocHistory",menu);
 }
}
