using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading;
using System.Text.RegularExpressions;
using System.Windows.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Xml.Linq;

internal sealed class Desktop {
 [DllImport("kagari_core",CharSet=CharSet.Unicode)] static extern IntPtr kagari_open(IntPtr owner);
 [DllImport("kagari_core")] static extern void kagari_close(IntPtr h);
 [DllImport("kagari_core",CharSet=CharSet.Unicode)] static extern IntPtr kagari_action(IntPtr h,int cmd,int index,string text);
 [DllImport("kagari_core")] static extern IntPtr kagari_rtf(IntPtr h,out int length);
 readonly Window window; IntPtr core; XElement data; bool drawing; string treeKey; int flags=-1;
 readonly string[] states={"未着手","作業中","確認待ち","完了"}, colors={"#9BAAB4","#368BA6","#DC9139","#3F9675"};
 readonly ImageSource[] icons=new ImageSource[2];
 static readonly string folder=AppDomain.CurrentDomain.BaseDirectory;
 T At<T>(string name) where T:FrameworkElement {return (T)window.FindName(name);}
 static Brush Color(string s){return (Brush)new BrushConverter().ConvertFromString(s);}
 static TextBlock Label(string s,double size){return new TextBlock{Text=s,FontSize=size,VerticalAlignment=VerticalAlignment.Center,TextTrimming=TextTrimming.CharacterEllipsis};}
 static ImageSource Icon(string name){return BitmapFrame.Create(new Uri(Path.Combine(folder,name)));}
 static string Value(XElement row,string field){return (string)row.Attribute(field)??"";}
 static string PathName(string p){return p.StartsWith(@"\\?\UNC\")?@"\\"+p.Substring(8):p.StartsWith(@"\\?\")?p.Substring(4):p;}
 static readonly Dictionary<string,DataTemplate> iconLabels=new Dictionary<string,DataTemplate>();
 static Button WithIcon(Button button,string glyph,ImageSource artwork=null){
  DataTemplate template;
  if(!iconLabels.TryGetValue(glyph,out template)){
   var row=new FrameworkElementFactory(typeof(StackPanel));row.SetValue(StackPanel.OrientationProperty,Orientation.Horizontal);
   FrameworkElementFactory icon;
   if(artwork==null){
    icon=new FrameworkElementFactory(typeof(TextBlock));icon.SetValue(TextBlock.TextProperty,glyph);
    icon.SetValue(TextBlock.FontFamilyProperty,new FontFamily("Segoe MDL2 Assets"));icon.SetValue(TextBlock.FontSizeProperty,15.0);
   }else{
    icon=new FrameworkElementFactory(typeof(Image));icon.SetValue(Image.SourceProperty,artwork);
    icon.SetValue(FrameworkElement.WidthProperty,18.0);icon.SetValue(FrameworkElement.HeightProperty,18.0);
   }
   icon.SetValue(FrameworkElement.VerticalAlignmentProperty,VerticalAlignment.Center);icon.SetValue(FrameworkElement.MarginProperty,new Thickness(0,0,5,0));
   var label=new FrameworkElementFactory(typeof(TextBlock));label.SetBinding(TextBlock.TextProperty,new System.Windows.Data.Binding());
   row.AppendChild(icon);row.AppendChild(label);template=new DataTemplate{VisualTree=row};iconLabels.Add(glyph,template);
  }
  button.ContentTemplate=template;return button;
 }
 static Button Action(string label,int id,string hint){
  var b=new Button{Content=label,Tag=id,ToolTip=hint};AutomationProperties.SetName(b,hint);
  if(id>=175 && id<=177)WithIcon(b,new[]{"\uE8A5","\uE81C","\uE73E"}[id-175]);
  if(id>=150 && id<=153)WithIcon(b,new[]{"\uE8FD","\uE787","\uE823","\uE7BA"}[id-150]);
  return b;
 }
 string previewText, previewPath;
 static readonly string activationDir=Path.Combine(Environment.GetEnvironmentVariable("LOCALAPPDATA")??Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"RustAIWorkspace","notification-clicks");
 static readonly Regex notificationUri=new Regex(@"\Akagari-task://open/([0-9]{1,20}-[0-9]{2})\z",RegexOptions.IgnoreCase|RegexOptions.CultureInvariant);
 FileSystemWatcher clicks;
 bool openingClick;
 static void QueueClick(string key){
  Directory.CreateDirectory(activationDir);
  string file=DateTime.UtcNow.Ticks.ToString("D19")+"_"+Guid.NewGuid().ToString("N")+"_"+key+".click";
  using(File.Create(Path.Combine(activationDir,file))){}
 }
 void StartClicks(){
  Directory.CreateDirectory(activationDir);
  clicks=new FileSystemWatcher(activationDir,"*.click");
  clicks.Created+=(sender,args)=>ScheduleClicks();
  clicks.Error+=(sender,args)=>ScheduleClicks();
  clicks.EnableRaisingEvents=true;
  ScheduleClicks();
 }
 void ScheduleClicks(){
  if(clicks==null || window.Dispatcher.HasShutdownStarted)return;
  window.Dispatcher.BeginInvoke(DispatcherPriority.Background,new Action(OpenClicks));
 }
 void OpenClicks(){
  if(openingClick || clicks==null || !window.IsEnabled || core==IntPtr.Zero)return;
  openingClick=true;
  try{
   foreach(string path in Directory.GetFiles(activationDir,"*.click").OrderBy(x=>x,StringComparer.Ordinal)){
    string key=Path.GetFileNameWithoutExtension(path).Split('_').Last();
    File.Delete(path);
    if(!notificationUri.IsMatch("kagari-task://open/"+key))continue;
    if(window.WindowState==WindowState.Minimized)window.WindowState=WindowState.Normal;
    window.Activate();
    if(Run(715,0,key))Run(160);
   }
  }finally{openingClick=false;}
 }
 internal Desktop(){
 using(var fs=File.OpenRead(Path.Combine(folder,"ui","Main.xaml"))){window=(Window)XamlReader.Load(fs);}
 window.Icon=At<Image>("Logo").Source=Icon("kagari.ico");
 At<Button>("About").Click+=(sender,args)=>{args.Handled=true;ShowAbout();};
 var askArt=Icon(@"assets\priority\ask-ai.png");
 WithIcon(At<Button>("Ask"),"ask-ai",askArt);WithIcon(At<Button>("OpenDocument"),"\uE8E5");
 WithIcon(At<Button>("ResetFilters"),"\uE894");WithIcon(At<Button>("Attach"),"\uE723");
 WithIcon(At<Button>("SharedProgress"),"\uE72D");WithIcon(At<Button>("NewTask"),"\uE710");
 AutomationProperties.SetName(At<Button>("Layout"),"ペインのレイアウト");
 AutomationProperties.SetName(At<Button>("AddProject"),"プロジェクトを追加");
 icons[0]=Icon(@"assets\codex.ico");icons[1]=Icon(@"assets\claude.ico");
 string[] glyphs={"\uE8B7","\uE721","\uE81C","\uE8AB","\uE8BD","\uE756"};
 string[] hints={"プロジェクト","タスク検索","セッション履歴","Git差分","AIに依頼","PowerShell"};
 int[] cmds={185,186,190,121,115,116};
 for(int i=0;i<cmds.Length;i++){var b=Action(glyphs[i],cmds[i],hints[i]);if(cmds[i]==115)b.Content=new Image{Source=askArt,Width=20,Height=20};b.FontFamily=new FontFamily("Segoe MDL2 Assets");b.FontSize=18;b.Width=40;b.Height=41;b.Foreground=Brushes.White;b.Background=Color("#294653");At<StackPanel>("Rail").Children.Add(b);}
 string[] tabs={"仕様書","作業ログ","結果"};for(int i=0;i<3;i++)At<WrapPanel>("Tabs").Children.Add(Action(tabs[i],175+i,tabs[i]));
 var preset=Action("",710,"縦3列　1:1:2（標準）");
 preset.Width=40;preset.Height=41;preset.Padding=new Thickness(7);preset.Background=Color("#294653");
 var columns=new Grid{Width=23,Height=16};
 for(int i=0;i<3;i++){
  columns.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(i==2?2:1,GridUnitType.Star)});
  var segment=new Border{Background=Color(i==2?"#AEDEDD":"#DFE9ED"),Margin=new Thickness(1,0,0,0),CornerRadius=new CornerRadius(1)};
  Grid.SetColumn(segment,i);columns.Children.Add(segment);
 }
 preset.Content=columns;
 At<StackPanel>("Rail").Children.Add(new Separator{Margin=new Thickness(4,6,4,6)});
 At<StackPanel>("Rail").Children.Add(preset);
 window.AddHandler(ButtonBase.ClickEvent,new RoutedEventHandler(Click));
 window.AddHandler(MenuItem.ClickEvent,new RoutedEventHandler(Click));
 At<TextBox>("Search").TextChanged+=(s,e)=>{if(!drawing)Run(703,0,At<TextBox>("Search").Text);};
 AutomationProperties.SetName(At<TextBox>("Search"),"タスク検索");
 var filters=At<WrapPanel>("AgentFilters");
 string[] agents={"全担当","Codex","Claude Code"};
 for(int i=0;i<agents.Length;i++){
  int index=i;
  FrameworkElement icon=i==0?(FrameworkElement)new TextBlock{Text="\uE716",FontFamily=new FontFamily("Segoe MDL2 Assets"),FontSize=18}:new Image{Source=icons[i-1],Width=20,Height=20};
  var identity=new StackPanel{Orientation=Orientation.Horizontal};identity.Children.Add(icon);
  var agentLabel=Label(agents[i],13);agentLabel.Margin=new Thickness(4,0,0,0);identity.Children.Add(agentLabel);
  var filter=new RadioButton{Content=identity,GroupName="QueueAgent",ToolTip=agents[i],Margin=new Thickness(0,2,8,2),MinHeight=32,VerticalContentAlignment=VerticalAlignment.Center};
  AutomationProperties.SetName(filter,agents[i]);filters.Children.Add(filter);
  filter.Checked+=(s,e)=>{if(!drawing)Run(707,index,"");};
 }
 window.PreviewKeyDown+=(s,e)=>{
  if(e.Key==Key.F && Keyboard.Modifiers==ModifierKeys.Control){At<TextBox>("Search").Focus();At<TextBox>("Search").SelectAll();e.Handled=true;}
  if(e.Key==Key.Escape && At<TextBox>("Search").IsKeyboardFocused){At<TextBox>("Search").Clear();e.Handled=true;}
 };
 var tasks=At<ListBox>("Tasks");tasks.SelectionChanged+=(s,e)=>{var row=tasks.SelectedItem as ListBoxItem;if(!drawing && row!=null)Run(702,(int)row.Tag,"");};
 tasks.MouseDoubleClick+=(s,e)=>{if(tasks.SelectedItem!=null)Run(160);};
 var tree=At<TreeView>("Tree");tree.SelectedItemChanged+=(s,e)=>{var item=e.NewValue as TreeViewItem;if(drawing||item==null)return;SelectExplorerItem(item);};
 At<ListBox>("DocumentHeadings").SelectionChanged+=(s,e)=>{
  var item=At<ListBox>("DocumentHeadings").SelectedItem as ListBoxItem;
  if(item!=null)((Paragraph)item.Tag).BringIntoView();
 };
 var docs=At<Border>("Docs");
 docs.SizeChanged+=(s,e)=>At<DockPanel>("DocumentOutline").Width=Math.Min(250,Math.Max(70,e.NewSize.Width*0.27));
 window.SourceInitialized+=(s,e)=>{core=kagari_open(new WindowInteropHelper(window).Handle);if(core==IntPtr.Zero)throw new InvalidOperationException("ワークスペースを読み込めません");Run(0);StartClicks();};
 window.Closed+=(s,e)=>{if(clicks!=null){clicks.Dispose();clicks=null;}if(core!=IntPtr.Zero)kagari_close(core);};
 }
 ContextMenu TaskMenu(int index,string title){
  var menu=new ContextMenu();
  var remove=new MenuItem{Header="タスクを削除…"};
  remove.Click+=(sender,args)=>{
   args.Handled=true;
   var answer=MessageBox.Show(window,"「"+title+"」を作業キューから削除しますか？\nセッション履歴とタスク資料のファイルは保持されます。",
    "タスクの削除",MessageBoxButton.YesNo,MessageBoxImage.Question,MessageBoxResult.No);
   if(answer==MessageBoxResult.Yes)Run(714,index);
  };
  menu.Items.Add(remove);
  return menu;
 }
 Button StatusButton(int task,int current,string title){
  var button=new Button{Content=states[current]+" ▾",Padding=new Thickness(6,2,6,2),FontSize=13,Margin=new Thickness(0,0,6,0),Background=Color(new[]{"#E6EBEF","#D6ECF3","#FBE7C9","#D8EDE2"}[current]),ToolTip="状態を変更"};
  AutomationProperties.SetName(button,title+" の状態を変更　"+states[current]);
  button.MouseDoubleClick+=(s,e)=>e.Handled=true;
  button.Click+=(s,e)=>{
   e.Handled=true;
   var menu=new ContextMenu{PlacementTarget=button,Placement=PlacementMode.Bottom};
   for(int n=0;n<states.Length;n++){
    int status=n;
    var item=new MenuItem{Header=states[n],IsCheckable=true,IsChecked=n==current};
    item.Click+=(sender,args)=>{args.Handled=true;if(status!=current)Run(711,task,status.ToString());};
    menu.Items.Add(item);
   }
   menu.IsOpen=true;
  };
  return button;
 }
 string DocumentProject(){
  var active=data.Elements("task").FirstOrDefault(t=>(int)t.Attribute("id")==(int)data.Attribute("task"));
  var selected=data.Elements("project").ElementAtOrDefault((int)data.Attribute("project"));
  return PathName(active!=null?Value(active,"project"):selected!=null?selected.Value:"");
 }
 void Click(object s,RoutedEventArgs e){
 var b=(e.OriginalSource as FrameworkElement);if(b==null)return;
 if(b.Name=="Layout"){LayoutMenu();return;}
 if(b.Name=="OpenDocument"){
  string project=DocumentProject();if(project.Length==0)return;
  string root=project.TrimEnd(Path.DirectorySeparatorChar)+Path.DirectorySeparatorChar;
  var picker=new Microsoft.Win32.OpenFileDialog{InitialDirectory=root,Filter="Markdown|*.md;*.markdown"};
  if(picker.ShowDialog(window)==true){
   if(picker.FileName.StartsWith(root,StringComparison.OrdinalIgnoreCase))Run(704,0,picker.FileName.Substring(root.Length));
   else MessageBox.Show(window,"選択中のプロジェクト内のMarkdownを選んでください。","KAGARI");
  }
  return;
 }
 if(b.Tag==null)return;string t=b.Tag.ToString();int id;
 if(t=="exit"){window.Close();return;}if(t=="log"){var f=new Microsoft.Win32.OpenFileDialog{Filter="ログ|*.log;*.txt|すべて|*.*"};if(f.ShowDialog(window)==true){Run(705,0,f.FileName);}return;}
 if(!int.TryParse(t,out id))return;
 if(id==185){if((flags&1)!=0)Run(194);At<TreeView>("Tree").Focus();return;}
 if(id==186){At<TextBox>("Search").Focus();At<TextBox>("Search").SelectAll();return;}
 if(id==121 && (flags&8)!=0)Run(197);Run(id);
 }
 bool Run(int id,int index=0,string text=""){
 if(core==IntPtr.Zero)return false;
 bool modal=new[]{103,113,115,127,160,190,192}.Contains(id);if(modal)window.IsEnabled=false;
 try{var ptr=kagari_action(core,id,index,text);var result=XElement.Parse(Marshal.PtrToStringUni(ptr));if(result.Name=="error"){MessageBox.Show(window,result.Value,"KAGARI");return false;}data=result;drawing=true;Paint(id);return true;}
 finally{drawing=false;if(modal){window.IsEnabled=true;window.Activate();ScheduleClicks();}}
 }
 static async System.Threading.Tasks.Task LoadPublishedVersion(TextBlock value,CancellationToken cancellation){
  try{
   using(var client=new System.Net.Http.HttpClient{Timeout=TimeSpan.FromSeconds(8)})
   using(var response=await client.GetAsync("https://raw.githubusercontent.com/norimaki1219-cmyk/KAGARI/main/Cargo.toml",cancellation)){
    response.EnsureSuccessStatusCode();
    string manifest=await response.Content.ReadAsStringAsync();
    var package=Regex.Match(manifest,@"(?ms)^\[package\]\s*$(.*?)(?=^\[|\z)");
    var version=Regex.Match(package.Groups[1].Value,@"(?m)^version\s*=\s*""([^""]+)""");
    value.Text=version.Success?"v"+version.Groups[1].Value:"バージョン情報を取得できません";
   }
  }catch(System.Net.Http.HttpRequestException){value.Text="取得できません（通信エラー）";}
   catch(OperationCanceledException){if(!cancellation.IsCancellationRequested)value.Text="取得できません（タイムアウト）";}
 }
 void ShowAbout(){
  var dialog=new Window{Owner=window,Title="バージョンと仕様",SizeToContent=SizeToContent.WidthAndHeight,ResizeMode=ResizeMode.NoResize,WindowStartupLocation=WindowStartupLocation.CenterOwner};
  var panel=new StackPanel{Margin=new Thickness(24),MinWidth=350};
  var title=Label("KAGARI（篝）",22);title.FontWeight=FontWeights.SemiBold;panel.Children.Add(title);
  var grid=new Grid{Margin=new Thickness(0,14,0,16)};
  grid.ColumnDefinitions.Add(new ColumnDefinition{Width=GridLength.Auto});grid.ColumnDefinitions.Add(new ColumnDefinition());
  string[,] specs={{"使用中のバージョン","v"+Value(data,"version")},{"最新公開版（main）","確認中…"},{"対応OS","Windows 11（x64）"},{"画面 / 処理","WPF / Rust"},{"対応AI","Codex・Claude Code"},{"セキュリティ連携","SHIRANUI ログの表示"}};
  TextBlock published=null;
  for(int i=0;i<specs.GetLength(0);i++){
   grid.RowDefinitions.Add(new RowDefinition{Height=GridLength.Auto});
   for(int col=0;col<2;col++){
    var cell=Label(specs[i,col],15);cell.Margin=new Thickness(0,4,16,4);
    Grid.SetRow(cell,i);Grid.SetColumn(cell,col);grid.Children.Add(cell);
    if(i==1 && col==1)published=cell;
   }
  }
  panel.Children.Add(grid);panel.Children.Add(WithIcon(new Button{Content="閉じる",IsCancel=true,MinWidth=82,HorizontalAlignment=HorizontalAlignment.Right},"\uE711"));
  dialog.Content=panel;
  using(var cancel=new CancellationTokenSource()){
   dialog.Loaded+=async(sender,args)=>await LoadPublishedVersion(published,cancel.Token);
   dialog.Closed+=(sender,args)=>cancel.Cancel();
   dialog.ShowDialog();
  }
 }
 void LayoutMenu(){var m=new ContextMenu();string[] names={"エクスプローラー","プロジェクト別の進捗","作業キュー","タスク資料","左右に分割","上下に分割","標準に戻す"};for(int i=0;i<7;i++){if(i==4||i==6)m.Items.Add(new Separator());int id=194+i;var item=new MenuItem{Header=names[i],IsCheckable=i<6,IsChecked=i<4?(flags&(1<<i))==0:i==4?(flags&16)==0:i==5&&(flags&16)!=0};item.Click+=(s,e)=>Run(id);m.Items.Add(item);}m.PlacementTarget=At<Button>("Layout");m.Placement=PlacementMode.Bottom;m.IsOpen=true;}
 void Paint(int action){
 if(action==704 || action==717){if(action==704)explorerPath=null;At<ScrollViewer>("ExplorerPreview").Visibility=Visibility.Collapsed;PaintDocument(action);return;}
 var projects=data.Elements("project").Select(p=>PathName(p.Value)).ToArray();int project=(int)data.Attribute("project");
 var search=At<TextBox>("Search");string query=Value(data,"query");if(!String.Equals(search.Text.Trim(),query,StringComparison.OrdinalIgnoreCase))search.Text=query;
 ((RadioButton)At<WrapPanel>("AgentFilters").Children[(int)data.Attribute("agent")]).IsChecked=true;string key=String.Join("|",projects);
 bool rebuilt=treeKey!=key;
 if(rebuilt){
  treeKey=key;var tree=At<TreeView>("Tree");tree.Items.Clear();
  projectNodes=projects.Select(ProjectNode).ToArray();
  foreach(int i in Enumerable.Range(0,projects.Length).OrderBy(j=>projects[j],StringComparer.OrdinalIgnoreCase)){
   var parent=Path.GetDirectoryName(projects[i]);
   int group=Array.FindIndex(projects,p=>String.Equals(p,parent,StringComparison.OrdinalIgnoreCase));
   if(group>=0)projectNodes[group].Items.Add(projectNodes[i]);else tree.Items.Add(projectNodes[i]);
  }
 }
 if(rebuilt || action==718){
  int startup=(int)data.Attribute("defaultProject");
  for(int i=0;i<projectNodes.Length;i++){
   var header=(StackPanel)projectNodes[i].Header;
   var badge=(TextBlock)header.Children[header.Children.Count-1];
   badge.Visibility=Visible(i==startup);
  }
 }
 bool taskDocument=action==715 || action==714 || rebuilt || action==701 || action==702 || action==127 || action==190 || (action>=175 && action<=177) || action==160 || action==113 || action==121;
 if(taskDocument){explorerPath=null;At<ScrollViewer>("ExplorerPreview").Visibility=Visibility.Collapsed;}
 if(taskDocument && project>=0 && project<projectNodes.Length)projectNodes[project].IsSelected=true;
 At<Button>("OpenDocument").IsEnabled=projects.Length>0;
 At<TextBlock>("ProjectLabel").Text=project<projects.Length?Path.GetFileName(projects[project].TrimEnd('\\')):"";
 var metrics=At<UniformGrid>("Metrics");metrics.Children.Clear();string[] names={"すべて","今週期限","確認待ち","期限超過"};int k=0;foreach(var n in data.Elements("metric")){var b=Action(names[k]+"   "+n.Value,150+k,names[k]);if(k==(int)data.Attribute("metric")){b.Background=Color("#D1E8E7");b.Foreground=Color("#115D60");}metrics.Children.Add(b);k++;}
 RenderProjectCharts(projects,project);
 var tasks=At<ListBox>("Tasks");int selected=(int)data.Attribute("task");
 var focused=tasks.SelectedItem as ListBoxItem;
 if(action!=702 || focused==null || (int)focused.Tag!=selected){
 tasks.Items.Clear();
 foreach(var t in data.Elements("task")){int i=(int)t.Attribute("id"),st=(int)t.Attribute("state");var row=new ListBoxItem{Tag=i,Padding=new Thickness(5,7,5,7)};var line=new DockPanel();var img=new Image{Source=icons[(int)t.Attribute("agent")],Width=19,Height=19,Margin=new Thickness(6,0,3,0),ToolTip=(int)t.Attribute("agent")==0?"Codex":"Claude Code"};DockPanel.SetDock(img,Dock.Right);line.Children.Add(img);var stack=new StackPanel();var title=Label(((bool)t.Attribute("pin")?"★  ":"")+Value(t,"title"),16);title.FontWeight=FontWeights.SemiBold;stack.Children.Add(title);var metadata=new WrapPanel{Margin=new Thickness(0,4,0,0)};
 if(project>=0 && project<projects.Length && IsCompany(projects[project])){
  string owner=PathName(Value(t,"project"));
  metadata.Children.Add(Label(IsCompany(owner)?".company 直下":Path.GetFileName(owner),13));
 }
 metadata.Children.Add(StatusButton(i,st,Value(t,"title")));
 var details=new[]{Value(t,"due"),Value(t,"reason")}.Where(v=>v!="" && v!=states[st] && v!="通常");
 metadata.Children.Add(Label(String.Join("　",details),13));stack.Children.Add(metadata);line.Children.Add(stack);row.Content=line;row.ContextMenu=TaskMenu(i,Value(t,"title"));AutomationProperties.SetName(row,Value(t,"title"));tasks.Items.Add(row);if(i==selected)row.IsSelected=true;}
 }
 var active=data.Elements("task").FirstOrDefault(t=>(int)t.Attribute("id")==selected);
 bool filtered=query.Length>0 || (int)data.Attribute("metric")!=0 || (int)data.Attribute("agent")!=0;
 At<TextBlock>("SearchHint").Visibility=Visible(search.Text.Length==0);
 At<Button>("ResetFilters").Visibility=Visible(filtered);
 At<StackPanel>("EmptyPanel").Visibility=Visible(tasks.Items.Count==0);
 bool noProject=projects.Length==0;
 At<TextBlock>("Empty").Text=noProject?"作業フォルダーを選んで始めます":filtered?"条件に一致するタスクがありません":"このプロジェクトにタスクはまだありません";
 var next=At<Button>("EmptyAction");next.Content=noProject?"フォルダーを選択":filtered?"全件を表示":"最初のタスクを作成";next.Tag=noProject?127:filtered?709:113;WithIcon(next,noProject?"\uE8B7":filtered?"\uE8FD":"\uE710");
 foreach(string name in new[]{"Advance","Pin","Ask","Details"})At<Button>(name).IsEnabled=active!=null;
 At<Button>("Advance").Content=active==null?"作業を開始":new[]{"作業を開始","確認待ちにする","完了にする","作業を再開"}[(int)active.Attribute("state")];
 WithIcon(At<Button>("Advance"),active==null?"\uE768":new[]{"\uE768","\uE823","\uE73E","\uE72C"}[(int)active.Attribute("state")]);
 At<Button>("Pin").Content=active!=null && (bool)active.Attribute("pin")?"★":"☆";
 PaintDocument(action);
 k=0;foreach(Button b in At<WrapPanel>("Tabs").Children){b.Background=Color(k++==(int)data.Attribute("doc")?"#D1E8E7":"#EEF2F4");}
 SetLayout((int)data.Attribute("panes"),action==710 || action==200);
 }
 void RefreshDocumentOutline(FlowDocument document){
  var list=At<ListBox>("DocumentHeadings");
  list.Items.Clear();
  // The native Markdown renderer assigns each heading level its own RTF point size.
  int[] sizes={34,32,30,28,27,26};
  foreach(var paragraph in document.Blocks.OfType<Paragraph>()){
   var start=paragraph.ContentStart.GetNextInsertionPosition(LogicalDirection.Forward);
   if(start==null || start.CompareTo(paragraph.ContentEnd)>=0)continue;
   var end=start.GetNextInsertionPosition(LogicalDirection.Forward);
   if(end==null)continue;
   var size=new TextRange(start,end).GetPropertyValue(TextElement.FontSizeProperty);
   if(!(size is double))continue;
   int depth=Array.IndexOf(sizes,(int)Math.Round((double)size*1.5));
   if(depth<0)continue;
   var title=new TextRange(paragraph.ContentStart,paragraph.ContentEnd).Text.Trim();
   if(title.Length==0)continue;
   var label=new TextBlock{Text=title,TextWrapping=TextWrapping.Wrap,Margin=new Thickness(6*depth,1,2,1)};
   var item=new ListBoxItem{Content=label,Tag=paragraph,HorizontalContentAlignment=HorizontalAlignment.Stretch};
   AutomationProperties.SetName(item,title);list.Items.Add(item);
  }
  At<DockPanel>("DocumentOutline").Visibility=Visible(list.Items.Count!=0);
 }
 void PaintDocument(int action){
 var pv=data.Element("preview");At<TextBlock>("FileName").Text=Value(pv,"path");At<Button>("Attach").Visibility=(bool)pv.Attribute("attach")?Visibility.Visible:Visibility.Collapsed;
 if(action==119 || action==704 || action==717 || previewText!=pv.Value || previewPath!=Value(pv,"path")){previewText=pv.Value;previewPath=Value(pv,"path");
 int length;IntPtr rtf=kagari_rtf(core,out length);var box=At<RichTextBox>("Preview");if(length>0){byte[] bytes=new byte[length];Marshal.Copy(rtf,bytes,0,length);using(var ms=new MemoryStream(bytes)){try{new TextRange(box.Document.ContentStart,box.Document.ContentEnd).Load(ms,DataFormats.Rtf);}catch(ArgumentException){box.Document=new FlowDocument(new Paragraph(new Run(pv.Value)));}}}
 RefreshDocumentOutline(box.Document);
 }
 if(explorerPath!=null){At<TextBlock>("FileName").Text=explorerPath;At<Button>("Attach").Visibility=Visibility.Collapsed;}
 }
 static readonly FontFamily explorerIcons=new FontFamily("Segoe MDL2 Assets");
 TreeViewItem ExplorerNode(string name,string path,object tag,string kind,string glyph){
  var header=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(0,1,0,1)};
  if(tag is int && !IsCompany(path)){
   var shape=new System.Windows.Shapes.Path{Data=(Geometry)window.FindResource("IconProject"),Stroke=(Brush)window.FindResource("ActionPrimary"),StrokeThickness=1.7,Stretch=Stretch.Uniform,Width=17,Height=17,Margin=new Thickness(2,0,6,0)};
   header.Children.Add(shape);
  }else header.Children.Add(new TextBlock{Text=glyph,FontFamily=explorerIcons,FontSize=16,Width=23,VerticalAlignment=VerticalAlignment.Center});
  var label=new TextBlock{Text=kind,FontSize=12,FontWeight=FontWeights.Normal,Foreground=SystemParameters.HighContrast?SystemColors.ControlTextBrush:Color(tag is int?"#15596A":"#475063")};
  if(!(tag is int) || IsCompany(path))header.Children.Add(new Border{Child=label,Padding=new Thickness(4,0,4,1),Margin=new Thickness(0,0,5,0),Background=SystemParameters.HighContrast?SystemColors.ControlBrush:Color(tag is int?"#D8EEF0":"#EAEAF1"),CornerRadius=new CornerRadius(2),VerticalAlignment=VerticalAlignment.Center});
  header.Children.Add(new TextBlock{Text=name,VerticalAlignment=VerticalAlignment.Center,FontWeight=tag is int?FontWeights.SemiBold:FontWeights.Normal});
  var item=new TreeViewItem{Header=header,Tag=tag,ToolTip=path};
  AutomationProperties.SetName(item,name+"（"+kind+"）");
  TextSearch.SetText(item,name);
  return item;
 }
 TreeViewItem FileNode(FileSystemInfo entry){
  string kind,glyph;
  string ext=entry.Extension.TrimStart('.').ToLowerInvariant();
  if(entry is DirectoryInfo){kind="フォルダー";glyph="\uE8B7";}
  else{
   if(ext.StartsWith("bak-",StringComparison.OrdinalIgnoreCase))ext="bak";
   switch(ext){
    case "md": case "markdown": kind="Markdown";glyph="\uE8A5";break;
    case "png":case "jpg":case "jpeg":case "gif":case "webp":case "svg":case "bmp":case "ico":case "tif":case "tiff":
     kind="画像";glyph="\uEB9F";break;
    case "mq5":case "mqh":case "rs":case "cs":case "cpp":case "h":case "c":case "py":case "js":case "jsx":case "ts":case "tsx":case "ps1":case "sh":case "html":case "css":case "xaml":
     kind="コード";glyph="\uE943";break;
    case "json":case "xml":case "yaml":case "yml":case "toml":case "ini":case "cfg":kind=ext.ToUpperInvariant();glyph="\uE713";break;
    case "db":case "sqlite":case "sqlite3":kind="DB";glyph="\uE964";break;
    case "bak":kind="バックアップ";glyph="\uE7C3";break;
    case "csv":case "tsv":case "xlsx":case "xls":kind="表";glyph="\uE80A";break;
    case "zip":case "7z":case "gz":case "rar":case "tar":kind="圧縮";glyph="\uE8B7";break;
    case "txt":case "log":kind=ext=="log"?"ログ":"テキスト";glyph="\uE8A5";break;
    default:kind=ext.Length==0?"ファイル":ext.ToUpperInvariant();glyph="\uE7C3";break;
   }
  }
  return ExplorerNode(entry.Name,entry.FullName,entry,kind,glyph);
 }
 // Each nonempty status occupies its fraction of a full turn, starting at twelve o'clock.
 Image StatusPie(int[] counts){
  var drawing=new DrawingVisual();
  var center=new Point(18,18);
  int total=counts.Sum(), cumulative=0;
  using(var dc=drawing.RenderOpen()){
   dc.DrawEllipse(Color("#E6ECEF"),null,center,18,18);
   for(int i=0;i<counts.Length;i++){
    if(counts[i]==0)continue;
    Brush fill=Color(colors[i]);
    if(counts[i]==total){dc.DrawEllipse(fill,null,center,18,18);break;}
    Point start=PiePoint(cumulative,total);
    cumulative+=counts[i];
    var wedge=new StreamGeometry();
    using(var path=wedge.Open()){
     path.BeginFigure(center,true,true);
     path.LineTo(start,true,false);
     path.ArcTo(PiePoint(cumulative,total),new Size(18,18),0,counts[i]>total/2.0,SweepDirection.Clockwise,true,false);
    }
    dc.DrawGeometry(fill,null,wedge);
   }
  }
  drawing.Drawing.Freeze();
  return new Image{Width=36,Height=36,Source=new DrawingImage(drawing.Drawing)};
 }
 static Point PiePoint(int count,int total){
  double angle=2*Math.PI*count/total-Math.PI/2;
  return new Point(18+18*Math.Cos(angle),18+18*Math.Sin(angle));
 }
 sealed class ProgressRow {
  public string Timestamp {get;set;}
  public int[] Counts {get;set;}
  public double[] Heights {get;set;}
  public string Detail {get;set;}
  public int Total {get {return Counts.Sum();}}
 }
 void OpenProgress(int project,string name){
  var history=XElement.Parse(Marshal.PtrToStringUni(kagari_action(core,716,project,"")));
  if(history.Name=="error"){MessageBox.Show(window,history.Value,"KAGARI");return;}
  var epoch=new DateTime(1970,1,1,0,0,0,DateTimeKind.Utc);
  var rows=history.Elements("point").Select(point=>new ProgressRow{
   Timestamp=epoch.AddMilliseconds((long)point.Attribute("time")).ToLocalTime().ToString("yyyy/MM/dd\nHH:mm:ss"),
   Counts=new[]{"a","b","c","d"}.Select(k=>(int)point.Attribute(k)).ToArray()
  }).ToArray();
  int peak=Math.Max(1,rows.Select(r=>r.Total).DefaultIfEmpty().Max());
  foreach(var row in rows){
   row.Heights=row.Counts.Select(v=>184.0*v/peak).ToArray();
   row.Detail=row.Timestamp.Replace("\n"," ")+"　"+String.Join("　",row.Counts.Select((n,i)=>states[i]+" "+n+"件"));
  }
  var dialog=new Window{Owner=window,Title=name+" の状態推移 — KAGARI",Width=840,Height=430,MinWidth=540,MinHeight=260,WindowStartupLocation=WindowStartupLocation.CenterOwner};
  var panel=new DockPanel{Margin=new Thickness(18)};
  var title=Label(name+" の状態推移",21);title.FontWeight=FontWeights.SemiBold;title.Foreground=Color("#176769");
  DockPanel.SetDock(title,Dock.Top);panel.Children.Add(title);
  var legend=new WrapPanel{Margin=new Thickness(0,6,0,8)};
  for(int i=0;i<states.Length;i++){
   var key=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(0,0,16,0)};
   key.Children.Add(new Border{Width=12,Height=12,Background=Color(colors[i]),Margin=new Thickness(0,0,4,0)});
   key.Children.Add(Label(states[i],14));legend.Children.Add(key);
  }
  DockPanel.SetDock(legend,Dock.Top);panel.Children.Add(legend);
  var close=new Button{Content="閉じる",IsCancel=true,HorizontalAlignment=HorizontalAlignment.Right,MinWidth=84,Margin=new Thickness(0,10,0,0)};
  WithIcon(close,"\uE711");DockPanel.SetDock(close,Dock.Bottom);panel.Children.Add(close);
  var chart=new ListBox{BorderThickness=new Thickness(0),HorizontalContentAlignment=HorizontalAlignment.Center};
  var strip=new FrameworkElementFactory(typeof(VirtualizingStackPanel));
  strip.SetValue(StackPanel.OrientationProperty,Orientation.Horizontal);
  chart.ItemsPanel=new ItemsPanelTemplate(strip);
  ScrollViewer.SetHorizontalScrollBarVisibility(chart,ScrollBarVisibility.Auto);
  VirtualizingStackPanel.SetVirtualizationMode(chart,VirtualizationMode.Recycling);
  var column=new FrameworkElementFactory(typeof(StackPanel));
  column.SetValue(FrameworkElement.WidthProperty,92.0);
  column.SetBinding(FrameworkElement.ToolTipProperty,new System.Windows.Data.Binding("Detail"));
  var plot=new FrameworkElementFactory(typeof(Grid));plot.SetValue(FrameworkElement.HeightProperty,212.0);
  var stack=new FrameworkElementFactory(typeof(StackPanel));stack.SetValue(FrameworkElement.VerticalAlignmentProperty,VerticalAlignment.Bottom);
  var total=new FrameworkElementFactory(typeof(TextBlock));total.SetValue(TextBlock.TextAlignmentProperty,TextAlignment.Center);
  total.SetBinding(TextBlock.TextProperty,new System.Windows.Data.Binding("Total"){StringFormat="{0} 件"});stack.AppendChild(total);
  for(int i=states.Length-1;i>=0;i--){
   var segment=new FrameworkElementFactory(typeof(Border));segment.SetValue(Border.BackgroundProperty,Color(colors[i]));
   segment.SetValue(FrameworkElement.WidthProperty,48.0);
   segment.SetBinding(FrameworkElement.HeightProperty,new System.Windows.Data.Binding("Heights["+i+"]"));stack.AppendChild(segment);
  }
  plot.AppendChild(stack);column.AppendChild(plot);
  var axis=new FrameworkElementFactory(typeof(Border));axis.SetValue(Border.BackgroundProperty,Color("#899BA3"));axis.SetValue(FrameworkElement.HeightProperty,1.0);column.AppendChild(axis);
  var date=new FrameworkElementFactory(typeof(TextBlock));date.SetValue(TextBlock.TextAlignmentProperty,TextAlignment.Center);
  date.SetBinding(TextBlock.TextProperty,new System.Windows.Data.Binding("Timestamp"));column.AppendChild(date);
  chart.ItemTemplate=new DataTemplate{VisualTree=column};chart.ItemsSource=rows;
  panel.Children.Add(chart);
  if(rows.Length>0)dialog.Loaded+=(sender,args)=>chart.ScrollIntoView(rows[rows.Length-1]);
  dialog.Content=panel;dialog.ShowDialog();
 }
 void RenderProjectCharts(string[] projects,int selected){
  var legend=At<WrapPanel>("ChartLegend");
  bool pinnedOnly=(bool)data.Attribute("pinnedOnly");
  if(legend.Children.Count==0){
   foreach(bool only in new[]{false,true}){
    var mode=new RadioButton{Content=only?"ピン留めのみ":"すべて表示",GroupName="ProjectChartMode",Margin=new Thickness(0,0,12,3)};
    mode.Click+=(sender,args)=>Run(713,only?1:0);
    legend.Children.Add(mode);
   }
   for(int i=0;i<states.Length;i++){
    var label=Label("● "+states[i],13);
    label.Foreground=Color(new[]{"#485E6C","#17627D","#935108","#226348"}[i]);label.Margin=new Thickness(0,0,12,3);
    legend.Children.Add(label);
   }
  }
  ((RadioButton)legend.Children[pinnedOnly?1:0]).IsChecked=true;
  var chart=At<WrapPanel>("Chart");chart.Children.Clear();
  var rows=data.Elements("project").ToArray();
  foreach(int index in Enumerable.Range(0,projects.Length).OrderBy(i=>projects[i],StringComparer.OrdinalIgnoreCase)){
   bool pinned=(bool)rows[index].Attribute("pinned");
   if(pinnedOnly && !pinned)continue;
   var counts=new[]{"a","b","c","d"}.Select(k=>(int)rows[index].Attribute(k)).ToArray();
   int total=counts.Sum();
   string path=projects[index],name=Path.GetFileName(path.TrimEnd(Path.DirectorySeparatorChar));
   if(IsCompany(path)){if(total==0)continue;name+="（直下）";}
   if(String.IsNullOrEmpty(name))name=path;
   var content=new StackPanel{Orientation=Orientation.Horizontal};
   content.Children.Add(StatusPie(counts));
   var caption=new StackPanel{Width=135,Margin=new Thickness(7,0,0,0),VerticalAlignment=VerticalAlignment.Center};
   caption.Children.Add(Label(name,14));
   caption.Children.Add(Label(total==0?"タスクなし":total+" 件",13));
   content.Children.Add(caption);
   string detail=name+"　"+String.Join("　",counts.Select((v,i)=>states[i]+" "+v+"件"));
   var button=new Button{Content=content,ToolTip=detail+"\n右クリックで状態推移を表示",Padding=new Thickness(5),Margin=new Thickness(1,2,4,2)};
   AutomationProperties.SetName(button,detail);
   if(index==selected)button.Background=Color("#D1E8E7");
   int projectIndex=index;
   button.Click+=(sender,args)=>Run(701,projectIndex,"");
   button.PreviewMouseUp+=(sender,args)=>{
    if(args.ChangedButton==MouseButton.Right){args.Handled=true;OpenProgress(projectIndex,name);}
   };
   button.KeyDown+=(sender,args)=>{
    if(args.Key==Key.Apps || (args.Key==Key.F10 && Keyboard.Modifiers==ModifierKeys.Shift)){
     args.Handled=true;OpenProgress(projectIndex,name);
    }
   };
   var tile=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(0,0,6,0)};
   tile.Children.Add(button);
   string hint=name+(pinned?" のピン留めを解除":" をピン留め");
   var pin=new Button{Content=pinned?"★":"☆",FontSize=17,Width=32,Padding=new Thickness(0),Margin=new Thickness(0,2,0,2),ToolTip=hint};
   AutomationProperties.SetName(pin,hint);
   pin.Click+=(sender,args)=>Run(712,projectIndex);
   tile.Children.Add(pin);
   chart.Children.Add(tile);
  }
  if(chart.Children.Count==0)chart.Children.Add(Label(pinnedOnly?"ピン留めしたプロジェクトはありません。「すべて表示」で ☆ を選んでください。":"プロジェクトがありません",14));
 }
 TreeViewItem ProjectNode(string path,int index){
  var name=Path.GetFileName(path.TrimEnd(Path.DirectorySeparatorChar));
  var node=ExplorerNode(String.IsNullOrEmpty(name)?path:name,path,index,IsCompany(path)?"会社":"プロジェクト",IsCompany(path)?"\uEC06":"\uE821");
  var badge=Label("既定",12);badge.Foreground=Color("#176769");badge.Margin=new Thickness(6,0,0,0);
  ((StackPanel)node.Header).Children.Add(badge);
  if(IsCompany(path))node.IsExpanded=true;else AddExpansion(node,new DirectoryInfo(path));
  var menu=new ContextMenu();
  var history=new MenuItem{Header="セッション履歴"};
  var startup=new MenuItem();
  menu.Opened+=(sender,args)=>{
   node.IsSelected=true;
   startup.Header=(int)data.Attribute("defaultProject")==index?"デフォルトプロジェクトを解除":"デフォルトプロジェクトに設定";
  };
  startup.Click+=(sender,args)=>{args.Handled=true;Run(718,index);};
  history.Click+=(sender,args)=>{args.Handled=true;node.IsSelected=true;if((int)data.Attribute("project")!=index)Run(701,index,"");Run(190);};
  menu.Items.Add(history);
  menu.Items.Add(startup);
  node.ContextMenu=menu;
  return node;
 }
 string explorerPath;
 static bool IsCompany(string path){return String.Equals(Path.GetFileName(path),".company",StringComparison.OrdinalIgnoreCase);}
 void AddExpansion(TreeViewItem node,DirectoryInfo directory){
  var placeholder=new TreeViewItem{IsEnabled=false,Header="読み込み中…"};
  node.Items.Add(placeholder);
  node.Expanded+=(s,e)=>{
   if(e.OriginalSource!=node || !node.Items.Contains(placeholder))return;
   node.Items.Remove(placeholder);
   try{
    var known=new HashSet<string>(node.Items.OfType<TreeViewItem>().Select(n=>n.ToolTip as string),StringComparer.OrdinalIgnoreCase);
    foreach(var entry in directory.EnumerateFileSystemInfos().OrderBy(f=>f is DirectoryInfo?0:1).ThenBy(f=>f.Name,StringComparer.OrdinalIgnoreCase)){
     if(known.Contains(entry.FullName))continue;
     var child=FileNode(entry);
     ContextMenuService.SetIsEnabled(child,false);
     if(entry is DirectoryInfo)AddExpansion(child,(DirectoryInfo)entry);
     node.Items.Add(child);
    }
   }catch(IOException ex){node.Items.Add(new TreeViewItem{Header=ex.Message,IsEnabled=false});}
    catch(UnauthorizedAccessException ex){node.Items.Add(new TreeViewItem{Header=ex.Message,IsEnabled=false});}
  };
 }
 void SelectExplorerItem(TreeViewItem node){
  var entry=node.Tag as FileSystemInfo;
  if(entry==null){
   int index=(int)node.Tag;
   Run(701,index,"");return;
  }
  if((flags&8)!=0)Run(197);
  string project=DocumentProject().TrimEnd(Path.DirectorySeparatorChar)+Path.DirectorySeparatorChar;
  if(entry is FileInfo && entry.FullName.StartsWith(project,StringComparison.OrdinalIgnoreCase) &&
     new[]{".md",".markdown"}.Contains(entry.Extension,StringComparer.OrdinalIgnoreCase)){
   Run(704,0,entry.FullName.Substring(project.Length));return;
  }
  if(entry is FileInfo && new[]{".md",".markdown"}.Contains(entry.Extension,StringComparer.OrdinalIgnoreCase)){
   var parent=node.Parent as TreeViewItem;
   while(parent!=null && !(parent.Tag is int))parent=parent.Parent as TreeViewItem;
   if(parent!=null){
    string root=PathName(data.Elements("project").ElementAt((int)parent.Tag).Value).TrimEnd(Path.DirectorySeparatorChar)+Path.DirectorySeparatorChar;
    if(entry.FullName.StartsWith(root,StringComparison.OrdinalIgnoreCase)){
     explorerPath=entry.FullName;Run(717,(int)parent.Tag,entry.FullName.Substring(root.Length));return;
    }
   }
  }
  explorerPath=entry.FullName;
  At<TextBlock>("FileName").Text=explorerPath;
  At<Button>("Attach").Visibility=Visibility.Collapsed;
  var view=At<ScrollViewer>("ExplorerPreview");
  view.Visibility=Visibility.Visible;
  try{
   var directory=entry as DirectoryInfo;
   if(directory!=null){
    var names=directory.EnumerateFileSystemInfos().Take(201).Select(f=>f.Name+(f is DirectoryInfo?" /":"")).ToArray();
    view.Content=PreviewLabel(names.Length==0?"空のフォルダーです。":String.Join(Environment.NewLine,names.Take(200))+(names.Length>200?"\n…":""));
   }else{
    var file=(FileInfo)entry;
    if(new[]{".png",".jpg",".jpeg",".gif",".bmp",".tif",".tiff"}.Contains(file.Extension,StringComparer.OrdinalIgnoreCase)){
     var image=new BitmapImage();using(var stream=file.OpenRead()){image.BeginInit();image.CacheOption=BitmapCacheOption.OnLoad;image.DecodePixelWidth=1024;image.StreamSource=stream;image.EndInit();}image.Freeze();
     view.Content=new Image{Source=image,Stretch=Stretch.Uniform,HorizontalAlignment=HorizontalAlignment.Left};
    }else{
     using(var stream=new FileStream(file.FullName,FileMode.Open,FileAccess.Read,FileShare.ReadWrite|FileShare.Delete))
     using(var reader=new StreamReader(stream,new System.Text.UTF8Encoding(false,true),true,4096)){
      char[] chars=new char[32769];int count=reader.ReadBlock(chars,0,chars.Length);
      string value=new string(chars,0,Math.Min(count,32768));
      view.Content=PreviewLabel(value.Any(c=>Char.IsControl(c)&&c!='\t'&&c!='\n'&&c!='\r')?FileDetails(file):value+(count==chars.Length?"\n…":""));
     }
    }
   }
  }catch(System.Text.DecoderFallbackException){view.Content=PreviewLabel(FileDetails((FileInfo)entry));}
   catch(IOException ex){view.Content=PreviewLabel(ex.Message);} catch(UnauthorizedAccessException ex){view.Content=PreviewLabel(ex.Message);} catch(NotSupportedException ex){view.Content=PreviewLabel(ex.Message);} catch(FileFormatException ex){view.Content=PreviewLabel(ex.Message);}
  view.ScrollToTop();
 }
 static TextBlock PreviewLabel(string value){return new TextBlock{Text=value,TextWrapping=TextWrapping.Wrap};}
 static string FileDetails(FileInfo f){return String.Format("{0}　{1:N0} バイト",f.Name,f.Length);}
 TreeViewItem[] projectNodes=new TreeViewItem[0];
 static Visibility Visible(bool b){return b?Visibility.Visible:Visibility.Collapsed;}
 void SetLayout(int f,bool reset){if(flags==f && !reset)return;flags=f;bool side=(f&1)==0,q=(f&4)==0,d=(f&8)==0,both=q&&d,vert=both&&(f&16)!=0;
 var ws=At<Grid>("Workspace");ws.ColumnDefinitions[0].Width=side?new GridLength(1,GridUnitType.Star):new GridLength(0);ws.ColumnDefinitions[2].Width=new GridLength(both && !vert?3:2,GridUnitType.Star);ws.ColumnDefinitions[1].Width=new GridLength(side?6:0);At<Border>("Side").Visibility=Visible(side);At<GridSplitter>("SideGrip").Visibility=Visible(side);
 At<Border>("Kpi").Visibility=Visible((f&2)==0);var panes=At<Grid>("Panes");var docs=At<Border>("Docs");var grip=At<GridSplitter>("Grip");At<Border>("Queue").Visibility=Visible(q);docs.Visibility=Visible(d);grip.Visibility=Visible(both);
 var zero=new GridLength(0);var star=new GridLength(1,GridUnitType.Star);panes.ColumnDefinitions[0].Width=star;panes.RowDefinitions[0].Height=star;panes.ColumnDefinitions[1].Width=both&&!vert?new GridLength(6):zero;panes.ColumnDefinitions[2].Width=both&&!vert?new GridLength(2,GridUnitType.Star):zero;panes.RowDefinitions[1].Height=vert?new GridLength(6):zero;panes.RowDefinitions[2].Height=vert?star:zero;
 Grid.SetColumn(docs,both&&!vert?2:0);Grid.SetRow(docs,vert?2:0);Grid.SetColumn(grip,both&&!vert?1:0);Grid.SetRow(grip,vert?1:0);grip.ResizeDirection=vert?GridResizeDirection.Rows:GridResizeDirection.Columns;
 }
 [STAThread] static int Main(string[] args){
 bool notification=args.Length>0 && args[0]=="--open-task";
 if(notification){
  if(args.Length!=2)return 2;
  var match=notificationUri.Match(args[1]);
  if(!match.Success)return 2;
  QueueClick(match.Groups[1].Value);
 }
 using(var mutex=new Mutex(false,@"Local\RustAIWorkspace_v1")){try{if(!mutex.WaitOne(0)){if(notification)return 0;MessageBox.Show("KAGARI は既に起動中です。");return 1;}}catch(AbandonedMutexException){}try{var app=new Application();var ui=new Desktop();if(args.Contains("--smoke")){ui.window.Loaded+=(s,e)=>{ui.window.UpdateLayout();ui.Run(199);if(ui.At<GridSplitter>("Grip").ResizeDirection!=GridResizeDirection.Rows)throw new Exception("上下分割");
 ui.Run(197);if(ui.At<Border>("Docs").Visibility!=Visibility.Collapsed)throw new Exception("資料非表示");
 ui.Run(200);if(ui.At<GridSplitter>("Grip").ResizeDirection!=GridResizeDirection.Columns)throw new Exception("左右分割");
 if(ui.data.Elements("task").Any()){ui.Run(702,(int)ui.data.Elements("task").First().Attribute("id"),"");if(ui.At<ListBox>("Tasks").SelectedItem==null)throw new Exception("タスク選択");}
 var filters=ui.At<WrapPanel>("AgentFilters");
 foreach(int index in new[]{1,2,0}){((RadioButton)filters.Children[index]).IsChecked=true;if((int)ui.data.Attribute("agent")!=index)throw new Exception("担当アイコンの絞り込み");}
 ui.Run(703,0,"__kagari_no_match__");
 if(ui.At<StackPanel>("EmptyPanel").Visibility!=Visibility.Visible || ui.At<Button>("Advance").IsEnabled)throw new Exception("空の検索結果");
 ui.Run(709);if(ui.At<TextBox>("Search").Text!="")throw new Exception("検索解除");
 ui.Run(199);ui.Run(197);ui.Run(194);ui.Run(710);
 var ws=ui.At<Grid>("Workspace");var panes=ui.At<Grid>("Panes");
 panes.ColumnDefinitions[0].Width=new GridLength(180);
 var rail=ui.At<StackPanel>("Rail");rail.Children.OfType<Button>().Single(b=>Convert.ToInt32(b.Tag)==710).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
 ui.window.UpdateLayout();
 double left=ws.ColumnDefinitions[0].ActualWidth,queue=panes.ColumnDefinitions[0].ActualWidth,docs=panes.ColumnDefinitions[2].ActualWidth;
 if(left<=0 || Math.Abs(left-queue)>3 || Math.Abs(2*queue-docs)>3 || ui.At<Border>("Side").Visibility!=Visibility.Visible)throw new Exception("縦3列プリセット");
 var tile=ui.At<WrapPanel>("Chart").Children.OfType<StackPanel>().FirstOrDefault();
 if(tile!=null){
  var pie=tile.Children.OfType<Button>().First();
  bool displayed=false;
  ui.window.Dispatcher.BeginInvoke(DispatcherPriority.ApplicationIdle,new Action(()=>{
   var modal=ui.window.OwnedWindows.Cast<Window>().Single();
   var list=((DockPanel)modal.Content).Children.OfType<ListBox>().Single();
   displayed=modal.IsVisible && list.Items.Count>0;modal.Close();
  }));
  pie.RaiseEvent(new MouseButtonEventArgs(Mouse.PrimaryDevice,Environment.TickCount,MouseButton.Right){RoutedEvent=Mouse.PreviewMouseUpEvent});if(!displayed)throw new Exception("状態推移");
 }
 if(ui.projectNodes.Length>=2){
  int original=(int)ui.data.Attribute("defaultProject");
  int chosen=original==1?0:1;
  var item=(MenuItem)ui.projectNodes[chosen].ContextMenu.Items[1];
  item.RaiseEvent(new RoutedEventArgs(MenuItem.ClickEvent));
  ui.Run(701,chosen==0?1:0);
  kagari_close(ui.core);
  ui.core=kagari_open(new WindowInteropHelper(ui.window).Handle);
  ui.treeKey=null;ui.Run(0);
  if((int)ui.data.Attribute("defaultProject")!=chosen || !ui.projectNodes[chosen].IsSelected)throw new Exception("既定プロジェクトの復元");
  ui.Run(718,chosen);
  if((int)ui.data.Attribute("defaultProject")!=-1)throw new Exception("既定プロジェクトの解除");
  if(original>=0)ui.Run(718,original);
 }
 string testRoot=Environment.GetEnvironmentVariable("AI_WORKSPACE_TEST_DIR");
 if(!String.IsNullOrEmpty(testRoot)){
  int fixture=Array.FindIndex(ui.data.Elements("project").ToArray(),p=>String.Equals(PathName(p.Value),testRoot,StringComparison.OrdinalIgnoreCase));
  if(fixture<0)throw new Exception("目次の検証用プロジェクト");
  ui.Run(701,fixture);ui.Run(704,0,"result.md");ui.window.UpdateLayout();
  var headings=ui.At<ListBox>("DocumentHeadings");
  var expected=File.ReadAllLines(Path.Combine(testRoot,"result.md")).Where(line=>Regex.IsMatch(line,@"^#{1,6} ")).Select(line=>line.TrimStart('#',' ')).ToArray();
  if(expected.Length==0 || !expected.SequenceEqual(headings.Items.Cast<ListBoxItem>().Select(item=>AutomationProperties.GetName(item))))throw new Exception("タスク資料の目次が本文の見出しと一致しません");
  headings.SelectedIndex=headings.Items.Count-1;ui.window.UpdateLayout();
  if(ui.At<RichTextBox>("Preview").ExtentHeight>ui.At<RichTextBox>("Preview").ViewportHeight && ui.At<RichTextBox>("Preview").VerticalOffset==0)throw new Exception("目次の移動");
 }
 bool aboutShown=false;
 ui.window.Dispatcher.BeginInvoke(DispatcherPriority.ApplicationIdle,new Action(()=>{
  var about=ui.window.OwnedWindows.Cast<Window>().Single();
  var info=((StackPanel)about.Content).Children.OfType<Grid>().Single();
  aboutShown=about.IsVisible && info.Children.OfType<TextBlock>().Any(cell=>cell.Text=="v"+Value(ui.data,"version"));
  about.Close();
 }));
 ui.At<Button>("About").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
 if(!aboutShown)throw new Exception("バージョンと仕様");
 ui.window.Close();};}app.Run(ui.window);return 0;}catch(Exception e){if(args.Contains("--smoke"))Console.Error.WriteLine(e);else MessageBox.Show(e.Message,"KAGARI");return 1;}finally{mutex.ReleaseMutex();}}}
}
