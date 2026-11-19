using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading;
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
 static Button Action(string label,int id,string hint){var b=new Button{Content=label,Tag=id,ToolTip=hint};AutomationProperties.SetName(b,hint);return b;}
 internal Desktop(){
 using(var fs=File.OpenRead(Path.Combine(folder,"ui","Main.xaml"))){window=(Window)XamlReader.Load(fs);}
 window.Icon=At<Image>("Logo").Source=Icon("kagari.ico");
 icons[0]=Icon(@"assets\codex.ico");icons[1]=Icon(@"assets\claude.ico");
 string[] glyphs={"\uE8B7","\uE721","\uE81C","\uE8AB","\uE8BD","\uE756"};
 string[] hints={"プロジェクト","タスク検索","セッション履歴","Git差分","AIに依頼","PowerShell"};
 int[] cmds={185,186,190,121,115,116};
 for(int i=0;i<cmds.Length;i++){var b=Action(glyphs[i],cmds[i],hints[i]);b.FontFamily=new FontFamily("Segoe MDL2 Assets");b.FontSize=18;b.Width=40;b.Height=41;b.Foreground=Brushes.White;b.Background=Color("#294653");At<StackPanel>("Rail").Children.Add(b);}
 string[] tabs={"仕様書","作業ログ","結果"};for(int i=0;i<3;i++)At<WrapPanel>("Tabs").Children.Add(Action(tabs[i],175+i,tabs[i]));
 window.AddHandler(ButtonBase.ClickEvent,new RoutedEventHandler(Click));
 window.AddHandler(MenuItem.ClickEvent,new RoutedEventHandler(Click));
 At<TextBox>("Search").TextChanged+=(s,e)=>{if(!drawing)Run(703,0,At<TextBox>("Search").Text);};
 AutomationProperties.SetName(At<TextBox>("Search"),"タスク検索");
 var agent=At<ComboBox>("Agent");agent.ItemsSource=new[]{"全担当","Codex","Claude Code"};agent.SelectionChanged+=(s,e)=>{if(!drawing)Run(707,agent.SelectedIndex,"");};
 var tasks=At<ListBox>("Tasks");tasks.SelectionChanged+=(s,e)=>{var row=tasks.SelectedItem as ListBoxItem;if(!drawing && row!=null)Run(702,(int)row.Tag,"");};
 tasks.MouseDoubleClick+=(s,e)=>{if(tasks.SelectedItem!=null)Run(160);};
 var tree=At<TreeView>("Tree");tree.AddHandler(TreeViewItem.ExpandedEvent,new RoutedEventHandler(Expand));tree.SelectedItemChanged+=(s,e)=>{var item=e.NewValue as TreeViewItem;if(drawing||item==null)return;var n=(Node)item.Tag;if(n.Index!=(int)data.Attribute("project"))Run(701,n.Index,"");if(File.Exists(n.Path) && new[]{".md",".markdown"}.Contains(Path.GetExtension(n.Path).ToLowerInvariant())){var root=PathName(data.Elements("project").ElementAt(n.Index).Value).TrimEnd('\\')+"\\";if(n.Path.StartsWith(root,StringComparison.OrdinalIgnoreCase))Run(704,0,n.Path.Substring(root.Length));}};
 window.SourceInitialized+=(s,e)=>{core=kagari_open(new WindowInteropHelper(window).Handle);if(core==IntPtr.Zero)throw new InvalidOperationException("ワークスペースを読み込めません");Run(0);};
 window.Closed+=(s,e)=>{if(core!=IntPtr.Zero)kagari_close(core);};
 }
 void Click(object s,RoutedEventArgs e){
 var b=(e.OriginalSource as FrameworkElement);if(b==null)return;
 if(b.Name=="Layout"){LayoutMenu();return;}
 if(b.Name=="Register"){var item=At<TreeView>("Tree").SelectedItem as TreeViewItem;if(item!=null){var n=(Node)item.Tag;if(Directory.Exists(n.Path))Run(708,0,n.Path);}return;}
 if(b.Tag==null)return;string t=b.Tag.ToString();int id;
 if(t=="exit"){window.Close();return;}if(t=="log"){var f=new Microsoft.Win32.OpenFileDialog{Filter="ログ|*.log;*.txt|すべて|*.*"};if(f.ShowDialog(window)==true){Run(705,0,f.FileName);}return;}
 if(!int.TryParse(t,out id))return;
 if(id==185){if((flags&1)!=0)Run(194);At<TreeView>("Tree").Focus();return;}
 if(id==186){At<TextBox>("Search").Focus();At<TextBox>("Search").SelectAll();return;}
 if(id==121 && (flags&8)!=0)Run(197);Run(id);
 }
 void Run(int id,int index=0,string text=""){
 if(core==IntPtr.Zero)return;
 bool modal=new[]{103,113,115,127,160,190,192}.Contains(id);if(modal)window.IsEnabled=false;
 try{var ptr=kagari_action(core,id,index,text);var result=XElement.Parse(Marshal.PtrToStringUni(ptr));if(result.Name=="error"){MessageBox.Show(window,result.Value,"KAGARI");return;}data=result;drawing=true;Paint();}
 finally{drawing=false;if(modal){window.IsEnabled=true;window.Activate();}}
 }
 void LayoutMenu(){var m=new ContextMenu();string[] names={"エクスプローラー","部署別の進捗","作業キュー","タスク資料","左右に分割","上下に分割","標準に戻す"};for(int i=0;i<7;i++){if(i==4||i==6)m.Items.Add(new Separator());int id=194+i;var item=new MenuItem{Header=names[i],IsCheckable=i<6,IsChecked=i<4?(flags&(1<<i))==0:i==4?(flags&16)==0:i==5&&(flags&16)!=0};item.Click+=(s,e)=>Run(id);m.Items.Add(item);}m.PlacementTarget=At<Button>("Layout");m.Placement=PlacementMode.Bottom;m.IsOpen=true;}
 void Paint(){
 var projects=data.Elements("project").Select(p=>PathName(p.Value)).ToArray();int project=(int)data.Attribute("project");
 var search=At<TextBox>("Search");string query=Value(data,"query");if(!String.Equals(search.Text.Trim(),query,StringComparison.OrdinalIgnoreCase))search.Text=query;
 At<ComboBox>("Agent").SelectedIndex=(int)data.Attribute("agent");string key=String.Join("|",projects);
 if(treeKey!=key){treeKey=key;var tree=At<TreeView>("Tree");tree.Items.Clear();for(int i=0;i<projects.Length;i++)tree.Items.Add(TreeNode(projects[i],i,true));}
 if(project<projects.Length){var root=(TreeViewItem)At<TreeView>("Tree").Items[project];var current=At<TreeView>("Tree").SelectedItem as TreeViewItem;if(current==null||((Node)current.Tag).Index!=project)root.IsSelected=true;}
 At<TextBlock>("ProjectLabel").Text=project<projects.Length?Path.GetFileName(projects[project].TrimEnd('\\')):"";
 var metrics=At<UniformGrid>("Metrics");metrics.Children.Clear();string[] names={"すべて","今週期限","確認待ち","期限超過"};int k=0;foreach(var n in data.Elements("metric")){var b=Action(names[k]+"   "+n.Value,150+k,names[k]);if(k==(int)data.Attribute("metric")){b.Background=Color("#D1E8E7");b.Foreground=Color("#115D60");}metrics.Children.Add(b);k++;}
 var chart=At<StackPanel>("Chart");chart.Children.Clear();
 foreach(var d in data.Elements("dept")){string name=Value(d,"name");var values=new[]{"a","b","c","d"}.Select(f=>(int)d.Attribute(f)).ToArray();var line=new DockPanel{Margin=new Thickness(0,2,0,2),Cursor=Cursors.Hand};var caption=Label(name==""?"部署未設定":name,12);caption.Width=88;DockPanel.SetDock(caption,Dock.Left);line.Children.Add(caption);var total=Label(values.Sum().ToString(),12);total.Width=32;total.TextAlignment=TextAlignment.Right;DockPanel.SetDock(total,Dock.Right);line.Children.Add(total);var bars=new Grid{Height=9,VerticalAlignment=VerticalAlignment.Center};for(int i=0;i<4;i++){bars.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(values[i],GridUnitType.Star)});var seg=new Border{Background=Color(colors[i]),CornerRadius=new CornerRadius(2)};Grid.SetColumn(seg,i);bars.Children.Add(seg);}line.Children.Add(bars);line.ToolTip=String.Join("　",values.Select((v,i)=>states[i]+" "+v));line.MouseLeftButtonUp+=(s,e)=>Run(706,name==""?1:0,name);chart.Children.Add(line);}
 if(chart.Children.Count==0)chart.Children.Add(Label("タスクを登録すると、部署ごとの進捗を表示します",12));
 else{var legend=new WrapPanel();for(int i=0;i<4;i++){var l=Label("● "+states[i],11);l.Foreground=Color(colors[i]);l.Margin=new Thickness(0,3,12,0);legend.Children.Add(l);}var clear=new Button{Content="全部署",Padding=new Thickness(5,1,5,1)};clear.Click+=(s,e)=>Run(706);legend.Children.Add(clear);chart.Children.Add(legend);}
 var tasks=At<ListBox>("Tasks");tasks.Items.Clear();int selected=(int)data.Attribute("task");
 foreach(var t in data.Elements("task")){int i=(int)t.Attribute("id"),st=(int)t.Attribute("state");var row=new ListBoxItem{Tag=i,Padding=new Thickness(5,7,5,7)};var line=new DockPanel();var img=new Image{Source=icons[(int)t.Attribute("agent")],Width=19,Height=19,Margin=new Thickness(6,0,3,0),ToolTip=(int)t.Attribute("agent")==0?"Codex":"Claude Code"};DockPanel.SetDock(img,Dock.Right);line.Children.Add(img);var stack=new StackPanel();var title=Label(((bool)t.Attribute("pin")?"★  ":"")+Value(t,"title"),14);title.FontWeight=FontWeights.SemiBold;stack.Children.Add(title);var meta=Label("● "+states[st]+"   "+Value(t,"dept")+"   "+Value(t,"due")+"   "+Value(t,"reason"),11);meta.Foreground=Color(colors[st]);stack.Children.Add(meta);line.Children.Add(stack);row.Content=line;AutomationProperties.SetName(row,Value(t,"title"));tasks.Items.Add(row);if(i==selected)row.IsSelected=true;}
 At<TextBlock>("Empty").Visibility=tasks.Items.Count==0?Visibility.Visible:Visibility.Hidden;At<Button>("Ask").IsEnabled=selected>=0;
 var pv=data.Element("preview");At<TextBlock>("FileName").Text=Value(pv,"path");At<Button>("Attach").Visibility=(bool)pv.Attribute("attach")?Visibility.Visible:Visibility.Collapsed;
 int length;IntPtr rtf=kagari_rtf(core,out length);var box=At<RichTextBox>("Preview");if(length>0){byte[] bytes=new byte[length];Marshal.Copy(rtf,bytes,0,length);using(var ms=new MemoryStream(bytes)){try{new TextRange(box.Document.ContentStart,box.Document.ContentEnd).Load(ms,DataFormats.Rtf);}catch(ArgumentException){box.Document=new FlowDocument(new Paragraph(new Run(pv.Value)));}}}
 k=0;foreach(Button b in At<WrapPanel>("Tabs").Children){b.Background=Color(k++==(int)data.Attribute("doc")?"#D1E8E7":"#EEF2F4");}
 SetLayout((int)data.Attribute("panes"));
 }
 class Node{internal string Path;internal int Index;internal bool Loaded;}
 TreeViewItem TreeNode(string path,int index,bool dir){var item=new TreeViewItem{Header=(dir?"▸  ":"▤  ")+(Path.GetFileName(path.TrimEnd('\\'))==""?path:Path.GetFileName(path.TrimEnd('\\'))),ToolTip=path,Tag=new Node{Path=path,Index=index}};if(dir)item.Items.Add("");return item;}
 void Expand(object s,RoutedEventArgs e){var item=e.OriginalSource as TreeViewItem;if(item==null)return;var n=(Node)item.Tag;if(n.Loaded)return;n.Loaded=true;item.Items.Clear();try{foreach(var d in Directory.EnumerateDirectories(n.Path).OrderBy(p=>p))item.Items.Add(TreeNode(d,n.Index,true));foreach(var f in Directory.EnumerateFiles(n.Path).OrderBy(p=>p))item.Items.Add(TreeNode(f,n.Index,false));}catch(IOException){}catch(UnauthorizedAccessException){}}
 static Visibility Visible(bool b){return b?Visibility.Visible:Visibility.Collapsed;}
 void SetLayout(int f){if(flags==f)return;flags=f;bool side=(f&1)==0,q=(f&4)==0,d=(f&8)==0,both=q&&d,vert=both&&(f&16)!=0;
 var ws=At<Grid>("Workspace");ws.ColumnDefinitions[0].Width=new GridLength(side?205:0);ws.ColumnDefinitions[1].Width=new GridLength(side?6:0);At<Border>("Side").Visibility=Visible(side);At<GridSplitter>("SideGrip").Visibility=Visible(side);
 At<Border>("Kpi").Visibility=Visible((f&2)==0);var panes=At<Grid>("Panes");var docs=At<Border>("Docs");var grip=At<GridSplitter>("Grip");At<Border>("Queue").Visibility=Visible(q);docs.Visibility=Visible(d);grip.Visibility=Visible(both);
 var zero=new GridLength(0);var star=new GridLength(1,GridUnitType.Star);panes.ColumnDefinitions[0].Width=star;panes.RowDefinitions[0].Height=star;panes.ColumnDefinitions[1].Width=both&&!vert?new GridLength(6):zero;panes.ColumnDefinitions[2].Width=both&&!vert?star:zero;panes.RowDefinitions[1].Height=vert?new GridLength(6):zero;panes.RowDefinitions[2].Height=vert?star:zero;
 Grid.SetColumn(docs,both&&!vert?2:0);Grid.SetRow(docs,vert?2:0);Grid.SetColumn(grip,both&&!vert?1:0);Grid.SetRow(grip,vert?1:0);grip.ResizeDirection=vert?GridResizeDirection.Rows:GridResizeDirection.Columns;
 }
 [STAThread] static int Main(string[] args){using(var mutex=new Mutex(false,@"Local\RustAIWorkspace_v1")){try{if(!mutex.WaitOne(0)){MessageBox.Show("KAGARI は既に起動中です。");return 1;}}catch(AbandonedMutexException){}try{var app=new Application();var ui=new Desktop();if(args.Contains("--smoke")){ui.window.Loaded+=(s,e)=>{ui.window.UpdateLayout();ui.Run(199);if(ui.At<GridSplitter>("Grip").ResizeDirection!=GridResizeDirection.Rows)throw new Exception("上下分割");
 ui.Run(197);if(ui.At<Border>("Docs").Visibility!=Visibility.Collapsed)throw new Exception("資料非表示");
 ui.Run(200);if(ui.At<GridSplitter>("Grip").ResizeDirection!=GridResizeDirection.Columns)throw new Exception("左右分割");
 if(ui.data.Elements("task").Any()){ui.Run(702,(int)ui.data.Elements("task").First().Attribute("id"),"");if(ui.At<ListBox>("Tasks").SelectedItem==null)throw new Exception("タスク選択");}
 ui.window.Close();};}app.Run(ui.window);return 0;}catch(Exception e){if(args.Contains("--smoke"))Console.Error.WriteLine(e);else MessageBox.Show(e.Message,"KAGARI");return 1;}finally{mutex.ReleaseMutex();}}}
}
