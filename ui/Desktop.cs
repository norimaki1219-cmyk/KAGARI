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

internal sealed partial class Desktop {
 [DllImport("kagari_core",CharSet=CharSet.Unicode)] static extern IntPtr kagari_open(IntPtr owner);
 [DllImport("kagari_core")] static extern void kagari_close(IntPtr h);
 [DllImport("kagari_core",CharSet=CharSet.Unicode)] static extern IntPtr kagari_action(IntPtr h,int cmd,int index,string text);
 [DllImport("kagari_core")] static extern IntPtr kagari_rtf(IntPtr h,out int length);
 readonly Window window; IntPtr core; XElement data; bool drawing; string treeKey; int flags=-1;
 static readonly string[] states={"未着手","作業中","確認待ち","完了"}, colors={"#9BAAB4","#368BA6","#DC9139","#3F9675"};
 const string followupState="次の作業依頼待ち";
 static readonly string[] progressStates=states.Concat(new[]{followupState}).ToArray();
 static readonly string[] progressColors=colors.Concat(new[]{"#9873BC"}).ToArray();
 static readonly string[] progressLegendColors={"#485E6C","#17627D","#935108","#226348","#6B4690"};
 static readonly string[] progressAttributes={"a","b","c","d","e"};
 static readonly int[] progressOrder={0,4,1,2,3};
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
  
  if(id>=150 && id<=153)WithIcon(b,new[]{"\uE8FD","\uE787","\uE823","\uE7BA"}[id-150]);
  return b;
 }
 string previewText, previewPath;
 static readonly string activationDir=Path.Combine(Environment.GetEnvironmentVariable("LOCALAPPDATA")??Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"RustAIWorkspace","notification-clicks");
 static readonly Regex notificationUri=new Regex(@"\Akagari-task://open/([0-9]{1,20}-[0-9]{2}(?:~[0-9a-f]{32})?)\z",RegexOptions.IgnoreCase|RegexOptions.CultureInvariant);
 FileSystemWatcher clicks;
 bool openingClick;
 bool nativeActionActive;
 readonly DispatcherTimer clickRetry=new DispatcherTimer{Interval=TimeSpan.FromMilliseconds(500)};
 sealed class NotificationBusyException:Exception {}
 static bool BusyResult(XElement result){return result.Name=="error" && result.Value=="処理中です";}
 static void QueueClick(string key){
  Directory.CreateDirectory(activationDir);
  string file=DateTime.UtcNow.Ticks.ToString("D19")+"_"+Guid.NewGuid().ToString("N")+"_"+key+".click";
  using(File.Create(Path.Combine(activationDir,file))){}
 }
 static readonly Regex guardNotificationUri=new Regex(@"\Akagari-alert://open/([1-9][0-9]{0,18})\z",RegexOptions.IgnoreCase|RegexOptions.CultureInvariant);
 static void RegisterGuardProtocol(){
  using(var key=Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Classes\kagari-alert")){
   key.SetValue("","URL:KAGARI alert");key.SetValue("URL Protocol","");
   using(var command=key.CreateSubKey(@"shell\open\command"))command.SetValue("","\""+typeof(Desktop).Assembly.Location+"\" --open-alert \"%1\"");
  }
 }
 void StartClicks(){
  RegisterGuardProtocol();
  clickRetry.Tick+=(s,e)=>{clickRetry.Stop();OpenClicks();};
  window.Activated+=(s,e)=>ScheduleClicks();
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
  if(openingClick || clickRetry.IsEnabled || clicks==null || core==IntPtr.Zero)return;
  var pending=Directory.GetFiles(activationDir,"*.click").OrderBy(x=>x,StringComparer.Ordinal).ToArray();
  if(pending.Length==0)return;
  if(nativeActionActive || drawing || !window.IsEnabled || ComponentDispatcher.IsThreadModal){
   clickRetry.Start();return;
  }
  openingClick=true;
  try{
   foreach(string path in pending){
    string key=Path.GetFileNameWithoutExtension(path).Split('_').Last();
    var alert=key.StartsWith("alert-",StringComparison.Ordinal)?guardNotificationUri.Match("kagari-alert://open/"+key.Substring(6)):Match.Empty;
    if(!alert.Success && !notificationUri.IsMatch("kagari-task://open/"+key)){File.Delete(path);continue;}
    if(window.WindowState==WindowState.Minimized)window.WindowState=WindowState.Normal;
    window.Activate();
    if(alert.Success)ShowGuard(alert.Groups[1].Value);
    else if(key.Contains("~"))RevealApproval(key);
    else if(Run(715,0,key,true))Run(160,0,"",true);
    File.Delete(path);
   }
  }catch(NotificationBusyException){clickRetry.Start();}finally{openingClick=false;}
 }
 static Desktop(){
  EventManager.RegisterClassHandler(typeof(ComboBox), FrameworkElement.SizeChangedEvent,
   new SizeChangedEventHandler((sender,e)=>{
    var combo=(ComboBox)sender;
    combo.Clip=new RectangleGeometry(new Rect(e.NewSize),6,6);
   }));
 }
 static Image AgentIcon(ImageSource source,double size){
  return new Image{Source=source,Width=size,Height=size,Clip=new EllipseGeometry(new Rect(0,0,size,size))};
 }
 internal Desktop(){
 using(var fs=typeof(Desktop).Assembly.GetManifestResourceStream("KAGARI.Main.xaml")){window=(Window)XamlReader.Load(fs);}
 if(Application.Current!=null){
  foreach(var type in new[]{typeof(TextBox),typeof(Button)})
   Application.Current.Resources[type]=window.Resources[type];
 }
 At<Image>("Logo").Source=Icon("kagari.ico");
 window.Icon=File.Exists(Path.Combine(folder,"assets","kagari-taskbar.ico")) ? Icon(Path.Combine("assets","kagari-taskbar.ico")) : At<Image>("Logo").Source;
 At<ComboBox>("QueueBatches").SelectionChanged+=(s,e)=>ShowQueueCounts();
 At<Button>("ViewRegistered").Click+=(s,e)=>{
  var receipt=SelectedReceipt();
  if(receipt==null || (int)receipt.Attribute("parent")<0)return;
  collapsedTasks.Remove((int)receipt.Attribute("parent"));
  if(Run(715,0,Value(receipt,"key"))){
   var list=At<ListBox>("Tasks");
   if(list.SelectedItem!=null)list.ScrollIntoView(list.SelectedItem);
  }
 };
 At<Button>("About").Click+=(sender,args)=>{args.Handled=true;ShowAbout();};
 var approvalPane=At<Border>("ApprovalPane");
 approvalPane.IsVisibleChanged+=(s,e)=>{if(approvalPane.IsVisible)RefreshApprovals();};
 At<ScrollViewer>("QueueColumn").SizeChanged+=(s,e)=>{
  var stack=At<Grid>("QueueStack");
  stack.Height=Math.Max(stack.MinHeight,e.NewSize.Height);
  At<ListBox>("ApprovalList").MaxHeight=Math.Min(200,Math.Max(110,stack.Height*0.38));
 };
 var askArt=Icon(@"assets\priority\ask-ai.png");
 WithIcon(At<Button>("Ask"),"ask-ai",askArt);WithIcon(At<Button>("OpenDocument"),"\uE8E5");
 WithIcon(At<Button>("ResetFilters"),"\uE894");WithIcon(At<Button>("Attach"),"\uE723");
 WithIcon(At<Button>("SharedProgress"),"\uE8A5");WithIcon(At<Button>("NewTask"),"\uE710");
 AutomationProperties.SetName(At<Button>("Layout"),"ペインのレイアウト");
 AutomationProperties.SetName(At<Button>("AddProject"),"プロジェクトを追加");
 icons[0]=Icon(@"assets\codex.ico");icons[1]=Icon(@"assets\claude.ico");
 var rail=At<StackPanel>("Rail");
 string[] art={null,"terminal",null,null,null,null,null,"panel-layout"};
 string[] hints={"プロジェクト","PowerShell","KPI分析","モデル培養","SHIRANUI","AIニュース","生成AI案件","縦3列　1:1:2（標準）"};
 int[] cmds={185,116,730,731,732,733,734,710};
 for(int i=0;i<cmds.Length;i++){
  if(cmds[i]==710)rail.Children.Add(new Border{Height=1,Background=Color("#52656D"),Margin=new Thickness(8,6,8,6)});
  var button=Action("",cmds[i],hints[i]);
  button.Style=(Style)window.FindResource("RailButton");
  if(cmds[i]==185){
   button.Content=new Image{Source=(ImageSource)window.FindResource("ProjectFolderIcon"),Width=23,Height=21};
  }else if(cmds[i]==730){
   analyticsButton=button;
   button.Content=new System.Windows.Shapes.Path{Width=21,Height=20,Stretch=Stretch.Fill,Fill=Brushes.White,
    Data=Geometry.Parse("M1,1 H3 V18 H22 V20 H1 Z M6,11 H9 V16 H6 Z M11,7 H14 V16 H11 Z M16,2 H19 V16 H16 Z")};
  }else if(cmds[i]==734){jobsButton=button;button.Content=new TextBlock{Text="\uE77B",FontFamily=new FontFamily("Segoe MDL2 Assets"),FontSize=21,Foreground=Brushes.White};}else if(cmds[i]==733){newsButton=button;button.Content=new TextBlock{Text="\uE900",FontFamily=new FontFamily("Segoe MDL2 Assets"),FontSize=21,Foreground=Brushes.White};}else if(cmds[i]==732){guardButton=button;button.Content=new TextBlock{Text="\uEA18",FontFamily=new FontFamily("Segoe MDL2 Assets"),FontSize=21,Foreground=Brushes.White};}else if(cmds[i]==731){cultureButton=button;button.Content=new System.Windows.Shapes.Path{Width=23,Height=22,Stretch=Stretch.Uniform,Fill=Brushes.White,Data=Geometry.Parse("M11,22 L11,15 C4,15 2,10 2,6 C8,6 12,9 12,13 C12,6 15,3 22,2 C22,9 19,13 14,14 L14,22 Z")};}else button.Content=new Image{Source=Icon(Path.Combine("assets","navigation",art[i]+".png")),Width=20,Height=20};
  ToolTipService.SetPlacement(button,PlacementMode.Right);
  rail.Children.Add(button);
 }
 var projectButton=(Button)rail.Children[0];
 var searchButton=(Button)rail.Children[1];
 ConfigureRailOrder(rail,Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"RustAIWorkspace","rail-order.txt"));
 Action<bool> focusSearch=selected=>{Selector.SetIsSelected(projectButton,!selected);Selector.SetIsSelected(searchButton,selected);};
 InitAnalytics();
 focusSearch(false);
 At<TreeView>("Tree").GotKeyboardFocus+=(s,e)=>focusSearch(false);
 At<TextBox>("Search").GotKeyboardFocus+=(s,e)=>{ShowWorkspace();focusSearch(true);};
 string[] tabs={"仕様書","作業ログ","結果"};for(int i=0;i<3;i++)At<WrapPanel>("Tabs").Children.Add(Action(tabs[i],175+i,tabs[i]));
 window.AddHandler(ButtonBase.ClickEvent,new RoutedEventHandler(Click));
 window.AddHandler(MenuItem.ClickEvent,new RoutedEventHandler(Click));
 At<TextBox>("Search").TextChanged+=(s,e)=>{if(!drawing)Run(703,0,At<TextBox>("Search").Text);};
 AutomationProperties.SetName(At<TextBox>("Search"),"タスク検索");
 var filters=At<WrapPanel>("AgentFilters");
 var bulkTools=(Panel)At<Button>("ResetFilters").Parent;
 bulkTools.Children.Add(bulkEdit);bulkTools.Children.Add(bulkUndo);
 bulkEdit.Click+=(s,e)=>OpenBulkEdit();
 bulkUndo.Click+=(s,e)=>Run(742);
 string[] agents={"全担当","Codex","Claude Code"};
 for(int i=0;i<agents.Length;i++){
  int index=i;
  FrameworkElement icon=i==0?(FrameworkElement)new TextBlock{Text="\uE716",FontFamily=new FontFamily("Segoe MDL2 Assets"),FontSize=18}:AgentIcon(icons[i-1],20);
  var identity=new StackPanel{Orientation=Orientation.Horizontal};identity.Children.Add(icon);
  var agentLabel=Label(agents[i],13);agentLabel.Margin=new Thickness(4,0,0,0);identity.Children.Add(agentLabel);
  var filter=new RadioButton{Content=identity,GroupName="QueueAgent",ToolTip=agents[i],Margin=new Thickness(0,2,8,2),MinHeight=32,VerticalContentAlignment=VerticalAlignment.Center};
  AutomationProperties.SetName(filter,agents[i]);filters.Children.Add(filter);
  filter.Checked+=(s,e)=>{if(!drawing)Run(707,index,"");};
 }
  var queueFilter=At<ComboBox>("QueueFilter");
  queueFilter.SelectionChanged+=(s,e)=>{
   if(!drawing && queueFilter.SelectedIndex>=0)Run(723,queueFilter.SelectedIndex);
  };
 window.PreviewKeyDown+=(s,e)=>{
  if(e.Key==Key.F && Keyboard.Modifiers==ModifierKeys.Control){At<TextBox>("Search").Focus();At<TextBox>("Search").SelectAll();e.Handled=true;}
  if(e.Key==Key.Escape && At<TextBox>("Search").IsKeyboardFocused){At<TextBox>("Search").Clear();e.Handled=true;}
 };
 var tasks=At<ListBox>("Tasks");tasks.SelectionChanged+=(s,e)=>{var row=tasks.SelectedItem as ListBoxItem;if(!drawing && row!=null)Run(702,(int)row.Tag,"");};
 tasks.MouseDoubleClick+=(s,e)=>{if(tasks.SelectedItem!=null)Run(160);};
 var tree=At<TreeView>("Tree");tree.SelectedItemChanged+=(s,e)=>{var item=e.NewValue as TreeViewItem;if(drawing||item==null)return;SelectExplorerItem(item);};
 At<ListBox>("DocumentHeadings").SelectionChanged+=(s,e)=>{
  var item=At<ListBox>("DocumentHeadings").SelectedItem as ListBoxItem;
  if(item!=null){At<Expander>("DocumentOutline").IsExpanded=false;((Paragraph)item.Tag).BringIntoView();At<ListBox>("DocumentHeadings").SelectedIndex=-1;}
 };
 WireProjectDocuments();
 kagari_attachment_preview(attachmentPreview);
 window.SourceInitialized+=(s,e)=>{core=kagari_open(new WindowInteropHelper(window).Handle);if(core==IntPtr.Zero)throw new InvalidOperationException("ワークスペースを読み込めません");Run(0);OpenProjectHome();StartClicks();StartUsage();StartQueueStatus();};
 window.Closed+=(s,e)=>{CancelTaskDocumentSearch();queueTimer.Stop();clickRetry.Stop();usageTimer.Stop();if(clicks!=null){clicks.Dispose();clicks=null;}if(core!=IntPtr.Zero)kagari_close(core);};
 }
 readonly Button bulkEdit=new Button{Content="一括変更"};
 readonly Button bulkUndo=new Button{Content="一括変更を取り消す"};
 void OpenBulkEdit(){
  var dialog=new Window{Owner=window,Title="タスクの一括変更",Width=520,Height=550,MinHeight=420,MinWidth=360,WindowStartupLocation=WindowStartupLocation.CenterOwner};
  var panel=new DockPanel{Margin=new Thickness(16)};dialog.Content=panel;
  var hint=Label("変更するタスクを選択（クリックで追加・解除）",14);
  DockPanel.SetDock(hint,Dock.Top);panel.Children.Add(hint);
  var options=new StackPanel{Margin=new Thickness(0,10,0,0)};
  DockPanel.SetDock(options,Dock.Bottom);panel.Children.Add(options);
  Func<string,string[],ComboBox> field=(label,values)=>{
   options.Children.Add(Label(label,14));var box=new ComboBox{Margin=new Thickness(0,0,0,4)};
   foreach(var value in values)box.Items.Add(value);
   box.SelectedIndex=0;options.Children.Add(box);return box;
  };
  var parent=field("親タスク",new[]{"変更しない","親なし"});
  var agent=field("担当",new[]{"変更しない","Codex","Claude Code"});
  var status=field("状態",new[]{"変更しない"}.Concat(states).ToArray());
  var buttons=new WrapPanel{HorizontalAlignment=HorizontalAlignment.Right};options.Children.Add(buttons);
  var cancel=new Button{Content="キャンセル",IsCancel=true};buttons.Children.Add(cancel);
  var apply=new Button{Content="0件に適用",IsEnabled=false};buttons.Children.Add(apply);
  var list=new ListBox{SelectionMode=SelectionMode.Multiple};panel.Children.Add(list);
  foreach(var t in data.Elements("task").Where(t=>Value(t,"context")!="true")){
   var item=new ListBoxItem{Content=ProjectName(PathName(Value(t,"project")))+"　"+Value(t,"title"),Tag=Value(t,"id")};
   list.Items.Add(item);
  }
  Func<string> selected=()=>String.Join(",",list.SelectedItems.Cast<ListBoxItem>().Select(item=>(string)item.Tag));
  Action update=()=>{
   int count=list.SelectedItems.Count;apply.Content=count+"件に適用";
   apply.IsEnabled=count>0 && (parent.SelectedIndex>0 || agent.SelectedIndex>0 || status.SelectedIndex>0);
  };
  list.SelectionChanged+=(s,e)=>{
   var previous=parent.SelectedItem as ComboBoxItem;
   int index=parent.SelectedIndex;
   while(parent.Items.Count>2)parent.Items.RemoveAt(parent.Items.Count-1);
   parent.SelectedIndex=index==1?1:0;
   if(list.SelectedItems.Count>0){
    var result=XElement.Parse(Marshal.PtrToStringUni(kagari_action(core,743,0,selected())));
    foreach(var p in result.Elements("parent")){
     var item=new ComboBoxItem{Content=p.Value,Tag=Value(p,"id")};parent.Items.Add(item);
     if(previous!=null && Equals(previous.Tag,item.Tag))parent.SelectedItem=item;
    }
   }
   update();
  };
  parent.SelectionChanged+=(s,e)=>update();agent.SelectionChanged+=(s,e)=>update();status.SelectionChanged+=(s,e)=>update();
  apply.Click+=(s,e)=>{
   string p=parent.SelectedIndex==0?"keep":parent.SelectedIndex==1?"root":(string)((ComboBoxItem)parent.SelectedItem).Tag;
   string a=agent.SelectedIndex==0?"keep":(agent.SelectedIndex-1).ToString();
   string st=status.SelectedIndex==0?"keep":(status.SelectedIndex-1).ToString();
   if(Run(741,0,String.Join("|",selected(),p,a,st)))dialog.DialogResult=true;
  };
  dialog.ShowDialog();
 }
 readonly HashSet<int> collapsedTasks=new HashSet<int>();
 string queueProject;
 bool queueFiltered;
 double? unfilteredOffset;
 static ScrollViewer QueueScroll(DependencyObject element){
  var scroll=element as ScrollViewer;
  if(scroll!=null)return scroll;
  for(int i=0;i<VisualTreeHelper.GetChildrenCount(element);i++){
   scroll=QueueScroll(VisualTreeHelper.GetChild(element,i));
   if(scroll!=null)return scroll;
  }
  return null;
 }
 void TaskProgress(StackPanel panel,XElement task){
  int total=(int)task.Attribute("children"), done=(int)task.Attribute("childrenDone"), state=(int)task.Attribute("state");
  string progress=total>0?String.Format("子タスク {0}/{1} 完了",done,total):"段階　"+states[state];
  int waiting=(int)task.Attribute("childrenWaiting");
  if(waiting>0)progress+="　確認待ち "+waiting+"件";
  var strip=new DockPanel{Margin=new Thickness(0,4,0,0)};
  bool showChildren=total>0 && done<total;
  var bar=new ProgressBar{Visibility=Visible(state!=3),Minimum=0,Maximum=showChildren?total:3,Value=showChildren?done:state,Width=48,Height=7,VerticalAlignment=VerticalAlignment.Center,Margin=new Thickness(0,0,8,0),
   ToolTip=showChildren?"直下の子タスクの完了割合（絞り込み前）":"状態に応じた段階。未着手 → 作業中 → 確認待ち → 完了"};
  AutomationProperties.SetName(bar,progress);DockPanel.SetDock(bar,Dock.Left);strip.Children.Add(bar);if(total>0){var count=Label(done+"/"+total,13);count.ToolTip=progress;count.Margin=new Thickness(0,0,8,0);DockPanel.SetDock(count,Dock.Left);strip.Children.Add(count);}
  bar.ToolTip=progress+"　"+(total>0 && !showChildren?"親タスク："+states[state]+"　":"")+bar.ToolTip;
  string condition=Value(task,"condition"),goal=Value(task,"goal");
  string summary=condition.Length>0?condition:goal.Length>0?goal:total>0?"子タスクの完了を確認（既定）":"未設定";
  var text=Label("完了条件　"+summary,13);text.TextWrapping=TextWrapping.NoWrap;text.TextTrimming=TextTrimming.CharacterEllipsis;text.ToolTip=summary;
  strip.Children.Add(text);panel.Children.Add(strip);
 }
 void EditCondition(XElement task){
  var dialog=new Window{Title="完了条件",Owner=window,Width=450,Height=185,WindowStartupLocation=WindowStartupLocation.CenterOwner};
  var box=new StackPanel{Margin=new Thickness(12)};dialog.Content=box;box.Children.Add(Label(Value(task,"title"),15));
  var field=new TextBox{Text=Value(task,"condition"),MaxLength=240,Margin=new Thickness(0,8,0,8)};box.Children.Add(field);
  var save=new Button{Content="保存",IsDefault=true,HorizontalAlignment=HorizontalAlignment.Right};
  box.Children.Add(save);save.Click+=(s,e)=>{if(Run(765,0,Value(task,"key")+"\n"+field.Text))dialog.DialogResult=true;};
  dialog.Loaded+=(s,e)=>{field.Focus();field.SelectAll();};dialog.ShowDialog();
 }
 ContextMenu TaskMenu(int index,string title){
  var menu=new ContextMenu();
  var child=new MenuItem{Header="＋ 子タスクを作成…"};
  child.Click+=(sender,args)=>{args.Handled=true;collapsedTasks.Remove(index);Run(720,index);};
  menu.Items.Add(child);
  var move=new MenuItem{Header="親タスクを変更"};
  move.Items.Add(new MenuItem());
  move.SubmenuOpened+=(sender,args)=>{
   if(args.OriginalSource!=move)return;
   move.Items.Clear();
   var root=new MenuItem{Header="最上位に移動"};
   root.Click+=(o,e)=>{e.Handled=true;Run(721,index,"root");};move.Items.Add(root);
   var parents=XElement.Parse(Marshal.PtrToStringUni(kagari_action(core,722,index,"")));
   foreach(var candidate in parents.Elements("parent")){
    int id=(int)candidate.Attribute("id");
    var item=new MenuItem{Header=candidate.Value};
    item.Click+=(o,e)=>{e.Handled=true;collapsedTasks.Remove(id);Run(721,index,id.ToString());};
    move.Items.Add(item);
   }
  };
  menu.Items.Add(move);
  var remove=new MenuItem{Header="タスクを削除…"};
  remove.Click+=(sender,args)=>{
   args.Handled=true;
   var answer=MessageBox.Show(window,"「"+title+"」を作業キューから削除しますか？\n子タスクはひとつ上の階層に移ります。セッション履歴とタスク資料は保持されます。",
    "タスクの削除",MessageBoxButton.YesNo,MessageBoxImage.Question,MessageBoxResult.No);
   if(answer==MessageBoxResult.Yes){collapsedTasks.Clear();Run(714,index);}
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
 if(t=="news"){ShowNews();return;}if(t=="guard"){ShowGuard();return;}if(t=="exit"){window.Close();return;}if(t=="log"){var f=new Microsoft.Win32.OpenFileDialog{Filter="ログ|*.log;*.txt|すべて|*.*"};if(f.ShowDialog(window)==true){Run(705,0,f.FileName);}return;}
 if(!int.TryParse(t,out id))return;
 if(id==734){ShowJobs();return;}
 if(id==733){ShowNews();return;}
 if(id==732){ShowGuard();return;}
 if(id==731){OpenCulture();return;}
 if(id==730){OpenAnalytics();return;}
 if(new[]{185,186,190,121,115,116,710}.Contains(id))ShowWorkspace();
 if(id==185){if((flags&1)!=0)Run(194);At<TreeView>("Tree").Focus();return;}
 if(id==186){At<TextBox>("Search").Focus();At<TextBox>("Search").SelectAll();return;}
 if(id==121 && (flags&8)!=0)Run(197);Run(id);
 }
 bool Run(int id,int index=0,string text="",bool notification=false){
 if(core==IntPtr.Zero)return false;
 if(nativeActionActive){if(notification)throw new NotificationBusyException();return false;}
 if(id==702 || id==746 || id==715 || id==701 || id==704 || id==717 || (id>=175 && id<=177))CancelTaskDocumentSearch();
 bool modal=new[]{103,113,115,127,160,190,192,720,747}.Contains(id);
 try{
  XElement result;nativeActionActive=true;
  try{if(modal)window.IsEnabled=false;result=XElement.Parse(Marshal.PtrToStringUni(kagari_action(core,id,index,text)));}
  finally{nativeActionActive=false;}
  if(notification && BusyResult(result))throw new NotificationBusyException();
  if(result.Name=="error"){MessageBox.Show(window,result.Value,"KAGARI");return false;}data=result;drawing=true;Paint(id);if(id==701 || id==127 || id==708)OpenProjectHome();else if(id==702 || id==746 || id==715)NavigateTaskDocument();return true;}
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
  var grid=new Grid{Margin=new Thickness(0,14,0,0)};
  grid.ColumnDefinitions.Add(new ColumnDefinition{Width=GridLength.Auto});grid.ColumnDefinitions.Add(new ColumnDefinition());
  string[,] specs={{"使用中のバージョン","v"+Value(data,"version")},{"最新公開版（main）","確認中…"},{"対応OS","Windows 11（x64）"},{"画面 / 処理","WPF / Rust"},{"対応AI","Codex・Claude Code"},{"セキュリティ連携","SHIRANUI 監視状態・アラート履歴"}};
  TextBlock published=null;
  for(int i=0;i<specs.GetLength(0);i++){
   grid.RowDefinitions.Add(new RowDefinition{Height=GridLength.Auto});
   for(int col=0;col<2;col++){
    var cell=Label(specs[i,col],15);cell.Margin=new Thickness(0,4,16,4);
    Grid.SetRow(cell,i);Grid.SetColumn(cell,col);grid.Children.Add(cell);
    if(i==1 && col==1)published=cell;
   }
  }
  panel.Children.Add(grid);
  dialog.PreviewKeyDown+=(sender,args)=>{if(args.Key==Key.Escape){args.Handled=true;dialog.Close();}};
  dialog.Content=panel;
  using(var cancel=new CancellationTokenSource()){
   dialog.Loaded+=async(sender,args)=>await LoadPublishedVersion(published,cancel.Token);
   dialog.Closed+=(sender,args)=>cancel.Cancel();
   dialog.ShowDialog();
  }
 }
 void LayoutMenu(){var m=new ContextMenu();string[] names={"エクスプローラー","プロジェクト別の進捗","作業キュー","タスク資料","左右に分割","上下に分割","標準に戻す"};for(int i=0;i<7;i++){if(i==4||i==6)m.Items.Add(new Separator());int id=194+i;var item=new MenuItem{Header=names[i],IsCheckable=i<6,IsChecked=i<4?(flags&(1<<i))==0:i==4?(flags&16)==0:i==5&&(flags&16)!=0};item.Click+=(s,e)=>Run(id);m.Items.Add(item);}m.PlacementTarget=At<Button>("Layout");m.Placement=PlacementMode.Bottom;m.IsOpen=true;}
 readonly DispatcherTimer queueTimer=new DispatcherTimer();
 XElement SelectedReceipt(){
  var item=At<ComboBox>("QueueBatches").SelectedItem as ComboBoxItem;
  return item==null?null:(XElement)item.Tag;
 }
 void ShowQueueCounts(){
  var r=SelectedReceipt();
  if(r==null)return;
  int added=(int)r.Attribute("added"),existing=(int)r.Attribute("existing"),failed=(int)r.Attribute("failed");
  var counts=At<TextBlock>("QueueCounts");
  counts.Text=String.Format("依頼 {0}件　新規 {1}件　登録済み {2}件　失敗 {3}件",added+existing+failed,added,existing,failed);
  counts.ToolTip=added==0?null:"新規登録したタスク\n"+String.Join("\n",r.Elements("added").Select(t=>t.Value));
  At<Button>("ViewRegistered").IsEnabled=(int)r.Attribute("parent")>=0;
 }
 int latestReceipt=-1;
 void PaintQueueResults(){
  var combo=At<ComboBox>("QueueBatches");
  var previous=SelectedReceipt();
  var receipts=data.Elements("receipt").ToArray();
  At<StackPanel>("QueueResults").Visibility=Visible(receipts.Length>0);
  combo.Items.Clear();
  if(receipts.Length==0){latestReceipt=-1;return;}
  int latest=(int)receipts[0].Attribute("id");
  int selected=latest==latestReceipt && previous!=null?(int)previous.Attribute("id"):latest;
  latestReceipt=latest;
  foreach(var r in receipts){
   int id=(int)r.Attribute("id");
   var item=new ComboBoxItem{Content="登録結果 #"+(id+1)+"　"+Value(r,"name"),Tag=r};
   combo.Items.Add(item);
   if(id==selected)item.IsSelected=true;
  }
  if(combo.SelectedIndex<0)combo.SelectedIndex=0;
 }
 readonly Dictionary<int,TextBlock> queueLabels=new Dictionary<int,TextBlock>();
  int[] queueRows=new int[0];
 void StartQueueStatus(){
  queueTimer.Interval=TimeSpan.FromSeconds(3);
  queueTimer.Tick+=(s,e)=>{
   if(!window.IsEnabled || drawing || nativeActionActive)return;
   bool progressVisible=At<WrapPanel>("Chart").IsVisible;
   if(At<Border>("ApprovalPane").IsVisible)RefreshApprovals();
   if(progressVisible || At<ListBox>("Tasks").IsVisible)RefreshQueueStatus(progressVisible);
  };
  RefreshApprovals();
  queueTimer.Start();
 }
 XElement ApprovalAction(int action,string payload){
  return XElement.Parse(Marshal.PtrToStringUni(kagari_action(core,action,0,payload)),LoadOptions.PreserveWhitespace);
 }
 void RevealApproval(string key){
  ShowWorkspace();
  if((flags&4)!=0 && !Run(196,0,"",true))return;
  if(!RefreshApprovals(true))return;
  var list=At<ListBox>("ApprovalList");
  var item=list.Items.Cast<ListBoxItem>().FirstOrDefault(row=>
   String.Equals(Value((XElement)row.Tag,"key"),key,StringComparison.OrdinalIgnoreCase));
  At<TextBlock>("ApprovalNotice").Text=item==null?"通知の承認要求は終了しました。":"";
  At<Border>("ApprovalPane").BringIntoView();
  if(item==null)return;
  list.SelectedItem=item;
  list.ScrollIntoView(item);
  item.Focus();
 }
 bool RefreshApprovals(bool notification=false){
  if(!window.IsEnabled || core==IntPtr.Zero)return false;
  var list=At<ListBox>("ApprovalList");
  var count=At<TextBlock>("ApprovalCount");
  var snapshot=ApprovalAction(760,"");
  if(notification && BusyResult(snapshot))throw new NotificationBusyException();
  if(snapshot.Name=="error"){
   list.IsEnabled=false;
   count.Text="取得できません";
   count.ToolTip=snapshot.Value;
   return false;
  }
  var rows=snapshot.Elements("approval").GroupBy(row=>Value(row,"key")).Select(group=>group.First()).ToArray();
  var existing=list.Items.Cast<ListBoxItem>().ToDictionary(item=>Value((XElement)item.Tag,"key"));
  var current=rows.ToDictionary(row=>Value(row,"key"));
  foreach(var item in existing){
   XElement row;
   if(!current.TryGetValue(item.Key,out row) || !XNode.DeepEquals((XElement)item.Value.Tag,row))
    list.Items.Remove(item.Value);
  }
  foreach(var row in rows){
   ListBoxItem item;
   if(!existing.TryGetValue(Value(row,"key"),out item) || !XNode.DeepEquals((XElement)item.Tag,row))
    list.Items.Add(ApprovalRow(row));
  }
  list.IsEnabled=true;
  list.Visibility=Visible(rows.Length>0);
  count.ToolTip=null;
  count.Text=rows.Length+" 件";
  return true;
 }
 ListBoxItem ApprovalRow(XElement row){
  var content=new StackPanel{Margin=new Thickness(4)};
  content.Children.Add(new TextBlock{Text=Value(row,"task"),FontWeight=FontWeights.Bold,TextWrapping=TextWrapping.Wrap});
  var buttons=new WrapPanel{Margin=new Thickness(0,6,0,4)};
  foreach(bool allow in new[]{true,false}){
   var button=new Button{
    Content=allow?"今回だけ許可":"拒否",
    Style=(Style)window.FindResource(allow?"PrimaryAction":"ApprovalDeny"),
    Margin=new Thickness(0,0,8,4),MinWidth=76,MinHeight=36,Padding=new Thickness(12,4,12,4),
    FontWeight=FontWeights.SemiBold,Cursor=Cursors.Hand
   };
   button.Click+=(s,e)=>{
    buttons.IsEnabled=false;
    var answer=ApprovalAction(allow?761:762,Value(row,"key")+"\n"+row.Value);
    var notice=At<TextBlock>("ApprovalNotice");
    notice.Text=answer.Name=="error"?answer.Value:"";
    if(answer.Name=="error")buttons.IsEnabled=true;
    RefreshApprovals();
   };
   buttons.Children.Add(button);
  }
  content.Children.Add(buttons);
  var details=new Expander{
   Style=(Style)window.FindResource("ApprovalDetails"),
   Content=new Border{
    Background=(Brush)window.FindResource("WorkspaceCanvas"),
    CornerRadius=new CornerRadius(4),Padding=new Thickness(8),Margin=new Thickness(0,4,0,0),
    Child=new TextBlock{Text=row.Value,FontSize=13,TextWrapping=TextWrapping.Wrap}
   }
  };
  AutomationProperties.SetName(details,"コマンドの詳細");
  content.Children.Add(details);
  return new ListBoxItem{Content=content,Tag=row};
 }
 void RefreshQueueStatus(bool includeProgress=false){
  if(core==IntPtr.Zero)return;
  var snapshot=XElement.Parse(Marshal.PtrToStringUni(kagari_action(core,719,includeProgress?1:0,"")));
   if(snapshot.Name!="statuses")return;
  if(Value(snapshot,"changed")=="1"){Run(0);return;}
   if(!drawing && (int)data.Attribute("queueFilter")!=0 &&
      !snapshot.Elements("status").Select(row=>(int)row.Attribute("id")).SequenceEqual(queueRows)){
    Run(0);return;
   }
  if(includeProgress){
   var projects=data.Elements("project").ToArray();
   bool repaint=false;
   foreach(var row in snapshot.Elements("progress")){
    var project=projects[(int)row.Attribute("id")];
    foreach(string key in new[]{"a","e"}){
     string count=Value(row,key);
     if(Value(project,key)==count)continue;
     project.SetAttributeValue(key,count);
     repaint=true;
    }
   }
   if(repaint)RenderProjectCharts(projects.Select(p=>PathName(p.Value)).ToArray(),(int)data.Attribute("project"));
  }
  foreach(var row in snapshot.Elements("status")){
   TextBlock label;
   if(!queueLabels.TryGetValue((int)row.Attribute("id"),out label))continue;
   string status=Value(row,"label");
   string detail=Value(row,"detail");

   bool problem=status.EndsWith("承認待ち") || status.EndsWith("実行失敗") || status.EndsWith("使用枠到達") || status.EndsWith("セッション使用中") || status.EndsWith("中断") || status.EndsWith("状態不明");
   label.ToolTip=problem?detail+"\n［タスク詳細・依頼］を開く":detail;
   string caption=status;
   if(label.Text==caption)continue;
   label.Text=caption;
   string color=status.EndsWith("実行中")?"#146884":status.EndsWith("回答受信")?"#267250":
    status.EndsWith("承認待ち")?"#895700":problem?"#AA432D":"#526671";
   label.Foreground=SystemParameters.HighContrast?SystemColors.WindowTextBrush:Color(color);
  }
 }
 void Paint(int action){
 if(action==704 || action==717){if(action==704)explorerPath=null;At<ScrollViewer>("ExplorerPreview").Visibility=Visibility.Collapsed;PaintDocument(action);return;}
 var projects=data.Elements("project").Select(p=>PathName(p.Value)).ToArray();int project=(int)data.Attribute("project");
 var search=At<TextBox>("Search");string query=Value(data,"query");if(!String.Equals(search.Text.Trim(),query,StringComparison.OrdinalIgnoreCase))search.Text=query;
  int queueFilter=(int)data.Attribute("queueFilter");
  At<ComboBox>("QueueFilter").SelectedIndex=queueFilter;
 ((RadioButton)At<WrapPanel>("AgentFilters").Children[(int)data.Attribute("agent")]).IsChecked=true;string key=String.Join("|",projects);
 bool rebuilt=treeKey!=key;
 if(rebuilt){
  treeKey=key;var tree=At<TreeView>("Tree");tree.Items.Clear();
  projectAliases.Clear();
  foreach(var row in data.Elements("project").Where(row=>Value(row,"alias")!=""))projectAliases[PathName(row.Value)]=Value(row,"alias");
  projectNodes=projects.Select(ProjectNode).ToArray();
  foreach(int i in Enumerable.Range(0,projects.Length).OrderBy(j=>projects[j],StringComparer.OrdinalIgnoreCase)){
   var parent=Path.GetDirectoryName(projects[i]);
   if(projectAliases.ContainsKey(projects[i]))while(parent!=null && !IsCompany(parent))parent=Path.GetDirectoryName(parent);
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
 bool taskDocument=action==715 || action==746 || action==714 || rebuilt || action==701 || action==702 || action==127 || action==190 || (action>=175 && action<=177) || action==160 || action==113 || action==121;
 if(taskDocument){explorerPath=null;At<ScrollViewer>("ExplorerPreview").Visibility=Visibility.Collapsed;}
 if(taskDocument && project>=0 && project<projectNodes.Length)projectNodes[project].IsSelected=true;
 At<Button>("OpenDocument").IsEnabled=projects.Length>0;
 At<Button>("GuardProject").Visibility=Visible(project>=0 && project<projects.Length && IsGuardProject(projects[project]));
 At<TextBlock>("ProjectLabel").Text=project<projects.Length?ProjectName(projects[project]):"";
 var metrics=At<UniformGrid>("Metrics");metrics.Children.Clear();string[] names={"すべて","今週期限","確認待ち","期限超過"};int k=0;foreach(var n in data.Elements("metric")){var b=Action(names[k]+"   "+n.Value,150+k,names[k]);if(k==(int)data.Attribute("metric")){b.Background=Color("#D1E8E7");b.Foreground=Color("#115D60");}metrics.Children.Add(b);k++;}
 RenderProjectCharts(projects,project);
 PaintQueueResults();
 var tasks=At<ListBox>("Tasks");int selected=(int)data.Attribute("task");
 bulkEdit.IsEnabled=data.Elements("task").Any();
 int undoCount=(int?)data.Element("bulkUndo")??0;
 bulkUndo.Visibility=Visible(undoCount>0);
 bulkUndo.Content="一括変更を取り消す（"+undoCount+"件）";
  bool filtered=query.Length>0 || (int)data.Attribute("metric")!=0 || (int)data.Attribute("agent")!=0 || queueFilter!=0;
 string ownerProject=projects.ElementAtOrDefault(project);
 double? restoreOffset=null;
 if(ownerProject!=queueProject)unfilteredOffset=null;
 else if(filtered!=queueFiltered){
  if(filtered){
   var scroll=QueueScroll(tasks);
   unfilteredOffset=scroll==null?(double?)null:scroll.VerticalOffset;
  }else{
   restoreOffset=unfilteredOffset;
   unfilteredOffset=null;
  }
 }
 queueFiltered=filtered;queueProject=ownerProject;
 var focused=tasks.SelectedItem as ListBoxItem;
  var rows=data.Elements("task").Select(row=>(int)row.Attribute("id")).ToArray();
  if(action!=702 || focused==null || (int)focused.Tag!=selected || !rows.SequenceEqual(queueRows)){
  queueRows=rows;
 queueLabels.Clear();tasks.Items.Clear();
 var all=data.Elements().Where(t=>t.Name=="task" || t.Name=="ancestor").ToArray();
 bool showReviewOrder=(int)data.Attribute("metric")==2;
 int reviewTotal=showReviewOrder?all.Count(t=>t.Name=="task" && (int)t.Attribute("state")==2):0;
 int reviewPosition=0;
 var byId=all.ToDictionary(t=>(int)t.Attribute("id"));
 var kpiGoals=data.Elements("kpiGoal").ToLookup(g=>(int)g.Attribute("task"));
 var children=all.ToLookup(t=>byId.ContainsKey((int)t.Attribute("parent"))?(int)t.Attribute("parent"):-1);
 if(action==715 || action==746 || action==720 || action==721){
  XElement current;
  int reveal=selected;
  while(byId.TryGetValue(reveal,out current)){reveal=(int)current.Attribute("parent");collapsedTasks.Remove(reveal);}
 }
 const double taskIndent=24, toggleWidth=25;
 bool hasTaskHierarchy=all.Any(t=>children.Contains((int)t.Attribute("id")));
 var pending=new Stack<Tuple<XElement,int>>();
 foreach(var root in children[-1].Reverse())pending.Push(Tuple.Create(root,0));
 while(pending.Count>0){
 var entry=pending.Pop();var t=entry.Item1;int depth=entry.Item2;
 int i=(int)t.Attribute("id");var row=new ListBoxItem{Tag=i,Padding=new Thickness(5,3,5,3)};var line=new DockPanel{Margin=new Thickness(Math.Min(depth,16)*taskIndent,0,0,0)};
 if(t.Name=="ancestor"){
  string caption="親タスク　"+Value(t,"title");
  var contextRow=new StackPanel();contextRow.Children.Add(Label(caption,15));TaskProgress(contextRow,t);line.Children.Add(contextRow);row.Content=line;row.IsEnabled=false;
  AutomationProperties.SetName(row,caption);tasks.Items.Add(row);
  foreach(var descendant in children[i].Reverse())pending.Push(Tuple.Create(descendant,depth+1));
  continue;
 }
 int st=(int)t.Attribute("state");
 var toggleSlot=new Border{Width=toggleWidth,Visibility=Visible(hasTaskHierarchy)};
 DockPanel.SetDock(toggleSlot,Dock.Left);line.Children.Add(toggleSlot);
 if(children.Contains(i)){
  bool expanded=filtered || !collapsedTasks.Contains(i);
  var toggle=new Button{Content=expanded?"▾":"▸",Width=toggleWidth,Height=24,Margin=new Thickness(0),Padding=new Thickness(0),VerticalAlignment=VerticalAlignment.Top,ToolTip=expanded?"子タスクを折りたたむ":"子タスクを展開"};
  AutomationProperties.SetName(toggle,Value(t,"title")+" の子タスク"+(expanded?"を折りたたむ":"を展開"));
  toggle.IsEnabled=!filtered;
  toggle.MouseDoubleClick+=(o,e)=>e.Handled=true;
  toggle.Click+=(o,e)=>{e.Handled=true;if(expanded){collapsedTasks.Add(i);Run(702,i);}else collapsedTasks.Remove(i);Run(0);};
  toggleSlot.Child=toggle;
  if(expanded)foreach(var descendant in children[i].Reverse())pending.Push(Tuple.Create(descendant,depth+1));
 }
 var img=AgentIcon(icons[(int)t.Attribute("agent")],19);img.Margin=new Thickness(6,0,3,0);img.ToolTip=(int)t.Attribute("agent")==0?"Codex":"Claude Code";DockPanel.SetDock(img,Dock.Right);line.Children.Add(img);var stack=new StackPanel();var title=Label(((bool)t.Attribute("pin")?"★  ":"")+Value(t,"title"),16);title.FontWeight=FontWeights.SemiBold;title.ToolTip=Value(t,"title");stack.Children.Add(title);var metadata=new WrapPanel();
 if(project>=0 && project<projects.Length && IsCompany(projects[project])){
  string owner=PathName(Value(t,"project"));
  metadata.Children.Add(Label(IsCompany(owner)?".company 直下":ProjectName(owner),13));
 }
 metadata.Children.Add(StatusButton(i,st,Value(t,"title")));
 string accessibleTitle=Value(t,"title");
 if(showReviewOrder && st==2){
  reviewPosition++;
  string order=String.Format("確認順 {0}/{1}",reviewPosition,reviewTotal);
  if(reviewPosition==1)order+="　最初に確認";
  var orderLabel=Label(order,13);
  orderLabel.FontWeight=FontWeights.SemiBold;
  metadata.Children.Add(orderLabel);
  accessibleTitle=order+"　"+accessibleTitle;
 }
 var linkedGoal=kpiGoals[i].FirstOrDefault();
 if(linkedGoal!=null){
  var review=new Button{Content="効果を確認",ToolTip="起票時と同じ範囲のKPIを開く"};
  review.Click+=(o,e)=>{e.Handled=true;OpenGoal(t,linkedGoal.Value);};
  metadata.Children.Add(review);
 }
 var ai=Label("",13);ai.Margin=new Thickness(7,0,3,0);ai.FontWeight=FontWeights.SemiBold;
 queueLabels.Add(i,ai);metadata.Children.Add(ai);
 var details=new[]{Value(t,"due"),Value(t,"reason")}.Where(v=>v!="" && v!=states[st] && v!="通常");
 metadata.Children.Add(Label(String.Join("　",details),13));stack.Children.Add(metadata);TaskProgress(stack,t);line.Children.Add(stack);row.Content=line;row.ContextMenu=TaskMenu(i,Value(t,"title"));var editCondition=new MenuItem{Header="完了条件を編集…"};editCondition.Click+=(o,e)=>EditCondition(t);row.ContextMenu.Items.Insert(0,editCondition);AutomationProperties.SetName(row,accessibleTitle);tasks.Items.Add(row);if(i==selected)row.IsSelected=true;}
 RefreshQueueStatus();
 }
 var active=data.Elements("task").FirstOrDefault(t=>(int)t.Attribute("id")==selected);
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
 SetLayout((int)data.Attribute("panes"),action==710 || action==200);
 if(restoreOffset.HasValue){
  tasks.UpdateLayout();
  var scroll=QueueScroll(tasks);
  if(scroll!=null)scroll.ScrollToVerticalOffset(restoreOffset.Value);
 }
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
  var outline=At<Expander>("DocumentOutline");
  outline.Visibility=Visible(list.Items.Count!=0);outline.Header="目次（"+list.Items.Count+"）";
 }
 void PaintDocument(int action){
 var pv=data.Element("preview");At<TextBlock>("FileName").Text=Value(pv,"path");At<Button>("Attach").Visibility=(bool)pv.Attribute("attach")?Visibility.Visible:Visibility.Collapsed;
 if(action==702 || action==715 || action==746 || action==119 || action==704 || action==717 || previewText!=pv.Value || previewPath!=Value(pv,"path")){previewText=pv.Value;previewPath=Value(pv,"path");
 int length;IntPtr rtf=kagari_rtf(core,out length);var box=At<RichTextBox>("Preview");if(length>0){byte[] bytes=new byte[length];Marshal.Copy(rtf,bytes,0,length);using(var ms=new MemoryStream(bytes)){try{new TextRange(box.Document.ContentStart,box.Document.ContentEnd).Load(ms,DataFormats.Rtf);}catch(ArgumentException){box.Document=new FlowDocument(new Paragraph(new Run(pv.Value)));}}}
 BindDocumentChecks(box,Value(pv,"path"));
 RefreshDocumentOutline(box.Document);
 }
 At<Expander>("DocumentOutline").Visibility=Visible(At<ListBox>("DocumentHeadings").Items.Count>0 && At<ScrollViewer>("ExplorerPreview").Visibility!=Visibility.Visible);
 RefreshDocumentContext();
 if(explorerPath!=null){At<TextBlock>("FileName").Text=explorerPath;At<Button>("Attach").Visibility=Visibility.Collapsed;}
 }
 readonly Dictionary<string,ImageBrush> explorerArt=new Dictionary<string,ImageBrush>();
 Border ExplorerIcon(string name,double size,bool rail=false){
  ImageBrush mask;
  if(!explorerArt.TryGetValue(name,out mask)){
   mask=new ImageBrush(Icon(Path.Combine("assets","explorer",name+".png")));mask.Freeze();explorerArt[name]=mask;
  }
  var icon=new Border{Width=size,Height=size,OpacityMask=mask,VerticalAlignment=VerticalAlignment.Center};
  if(rail)icon.Background=Brushes.White;
  else if(SystemParameters.HighContrast)icon.SetBinding(Border.BackgroundProperty,new System.Windows.Data.Binding("Foreground"){RelativeSource=new System.Windows.Data.RelativeSource(System.Windows.Data.RelativeSourceMode.FindAncestor,typeof(TreeViewItem),1)});
  else icon.Background=(Brush)window.FindResource("ActionPrimary");
  return icon;
 }
 static readonly FontFamily explorerIcons=new FontFamily("Segoe MDL2 Assets");
 TreeViewItem ExplorerNode(string name,string path,object tag,string kind,string glyph){
  var header=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(0,1,0,1)};
  string artwork=tag is int && !IsCompany(path)?"project":tag is DirectoryInfo?"folder":kind=="Markdown"?"markdown-file":null;
  if(artwork!=null){
   var icon=ExplorerIcon(artwork,18);icon.Margin=new Thickness(1,0,5,0);header.Children.Add(icon);
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
 Image StatusPie(int[] counts,string[] palette=null){
  var drawing=new DrawingVisual();
  var center=new Point(18,18);
  int total=counts.Sum(), cumulative=0;
  using(var dc=drawing.RenderOpen()){
   dc.DrawEllipse(Color("#E6ECEF"),null,center,18,18);
   for(int i=0;i<counts.Length;i++){
    if(counts[i]==0)continue;
    Brush fill=Color((palette??colors)[i]);
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
 static void ArrangeRail(StackPanel rail,List<Button> buttons,Border separator){
  rail.Children.Clear();
  foreach(var button in buttons){
   if((int)button.Tag==710 && rail.Children.Count>0)rail.Children.Add(separator);
   rail.Children.Add(button);
  }
 }
 static void ConfigureRailOrder(StackPanel rail,string path){
  var buttons=rail.Children.OfType<Button>().ToList();
  var separator=rail.Children.OfType<Border>().Single();
  try{
   if(File.Exists(path)){
    var saved=File.ReadAllLines(path);
    buttons=buttons.OrderBy(b=>{
     int index=Array.IndexOf(saved,b.Tag.ToString());
     return index<0?int.MaxValue:index;
    }).ToList();
    ArrangeRail(rail,buttons,separator);
   }
  }catch(IOException){}catch(UnauthorizedAccessException){}
  const string railFormat="KAGARI.RailButton";
  Button dragging=null;
  foreach(var button in buttons.ToArray()){
   Point? press=null;
   button.PreviewMouseLeftButtonDown+=(s,e)=>press=e.GetPosition(rail);
   button.PreviewMouseLeftButtonUp+=(s,e)=>press=null;
   button.LostMouseCapture+=(s,e)=>press=null;
   button.PreviewMouseMove+=(s,e)=>{
    if(!press.HasValue || e.LeftButton!=MouseButtonState.Pressed)return;
    var delta=e.GetPosition(rail)-press.Value;
    if(Math.Abs(delta.X)<SystemParameters.MinimumHorizontalDragDistance &&
       Math.Abs(delta.Y)<SystemParameters.MinimumVerticalDragDistance)return;
    press=null;
    button.ReleaseMouseCapture();
    dragging=button;e.Handled=true;
    try{DragDrop.DoDragDrop(button,new DataObject(railFormat,(int)button.Tag),DragDropEffects.Move);}
    finally{dragging=null;}
   };
   button.AllowDrop=true;
   button.PreviewDragOver+=(s,e)=>{
    e.Effects=dragging!=null && dragging!=button && e.Data.GetDataPresent(railFormat,false)?DragDropEffects.Move:DragDropEffects.None;
    e.Handled=true;
   };
   button.PreviewDrop+=(s,e)=>{
    e.Handled=true;e.Effects=DragDropEffects.None;
    if(dragging==null || dragging==button || !e.Data.GetDataPresent(railFormat,false))return;
    int to=buttons.IndexOf(button);
    var next=new List<Button>(buttons);
    next.Remove(dragging);next.Insert(to,dragging);
    string temp=path+"."+Guid.NewGuid().ToString("N")+".tmp";
    try{
     Directory.CreateDirectory(Path.GetDirectoryName(path));
     File.WriteAllLines(temp,next.Select(b=>b.Tag.ToString()));
     if(File.Exists(path))File.Replace(temp,path,null);else File.Move(temp,path);
    }catch(IOException ex){MessageBox.Show(ex.Message,"並び順を保存できません");return;}
     catch(UnauthorizedAccessException ex){MessageBox.Show(ex.Message,"並び順を保存できません");return;}
    buttons=next;
    ArrangeRail(rail,buttons,separator);
    e.Effects=DragDropEffects.Move;
   };
  }
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
 static ProgressRow[] DailyProgressRows(XElement history){
  var epoch=new DateTime(1970,1,1,0,0,0,DateTimeKind.Utc);
  var latest=history.Elements("point").OrderBy(p=>(long)p.Attribute("time"))
   .Select(p=>new{Point=p,Time=epoch.AddMilliseconds((long)p.Attribute("time")).ToLocalTime()})
   .GroupBy(p=>p.Time.Date).ToDictionary(day=>day.Key,day=>day.Last());
  var rows=new List<ProgressRow>();
  if(latest.Count==0)return rows.ToArray();
  var first=latest.Keys.Min();
  var end=latest.Keys.Max();
  if(end<DateTime.Today)end=DateTime.Today;
  int[] counts=null;
  for(var day=first;;day=day.AddDays(1)){
   var stamp=day.ToString("yyyy/MM/dd")+"\n前日から継続";
   if(latest.ContainsKey(day)){
    var observation=latest[day];
    counts=new[]{"a","b","c","d"}.Select(k=>(int)observation.Point.Attribute(k)).ToArray();
    stamp=observation.Time.ToString("yyyy/MM/dd\nHH:mm:ss");
   }
   rows.Add(new ProgressRow{Timestamp=stamp,Counts=counts});
   if(day==end)break;
  }
  return rows.ToArray();
 }
 void OpenProgress(int project,string name){
  var history=XElement.Parse(Marshal.PtrToStringUni(kagari_action(core,716,project,"")));
  if(history.Name=="error"){MessageBox.Show(window,history.Value,"KAGARI");return;}
  ShowProgressHistory(name,history);
 }
 void ShowProgressHistory(string name,XElement history){
  var rows=DailyProgressRows(history);
  int peak=Math.Max(1,rows.Select(r=>r.Total).DefaultIfEmpty().Max());
  var historyStates=(string[])states.Clone();
  historyStates[0]="未着手・次の作業依頼待ち（合計）";
  foreach(var row in rows){
   row.Heights=row.Counts.Select(v=>184.0*v/peak).ToArray();
   row.Detail=row.Timestamp.Replace("\n"," ")+"　"+String.Join("　",row.Counts.Select((n,i)=>historyStates[i]+" "+n+"件"));
  }
  var dialog=new Window{Owner=window,Title=name+" の状態推移 — KAGARI",Width=840,Height=430,MinWidth=540,MinHeight=260,WindowStartupLocation=WindowStartupLocation.CenterOwner};
  var panel=new DockPanel{Margin=new Thickness(18)};
  var title=Label(name+" の状態推移",21);title.FontWeight=FontWeights.SemiBold;title.Foreground=Color("#176769");
  DockPanel.SetDock(title,Dock.Top);panel.Children.Add(title);
  var legend=new WrapPanel{Margin=new Thickness(0,6,0,8)};
  for(int i=0;i<states.Length;i++){
   var key=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(0,0,16,0)};
   key.Children.Add(new Border{Width=12,Height=12,Background=Color(colors[i]),Margin=new Thickness(0,0,4,0)});
   key.Children.Add(Label(historyStates[i],14));legend.Children.Add(key);
  }
  DockPanel.SetDock(legend,Dock.Top);panel.Children.Add(legend);
  var close=new Button{Content="閉じる",IsCancel=true,HorizontalAlignment=HorizontalAlignment.Right,MinWidth=84,Margin=new Thickness(0,10,0,0)};
  WithIcon(close,"\uE711");DockPanel.SetDock(close,Dock.Bottom);panel.Children.Add(close);
  var chart=new ListBox{BorderThickness=new Thickness(0),HorizontalContentAlignment=HorizontalAlignment.Center};
  var strip=new FrameworkElementFactory(typeof(VirtualizingStackPanel));
  strip.SetValue(VirtualizingStackPanel.OrientationProperty,Orientation.Horizontal);
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
   foreach(int i in progressOrder){
    var label=Label("● "+progressStates[i],13);
    label.Foreground=Color(progressLegendColors[i]);label.Margin=new Thickness(0,0,12,3);
    if(i==4)label.ToolTip="未着手で登録されており、直近の依頼に正常な回答があり、実行中の依頼がないタスク";
    legend.Children.Add(label);
   }
  }
  ((RadioButton)legend.Children[pinnedOnly?1:0]).IsChecked=true;
  var chart=At<WrapPanel>("Chart");chart.Children.Clear();
  var rows=data.Elements("project").ToArray();
  var buttonStyle=(Style)window.Resources["ProjectCardButton"];
  foreach(int index in Enumerable.Range(0,projects.Length).OrderBy(i=>projects[i],StringComparer.OrdinalIgnoreCase)){
   bool pinned=(bool)rows[index].Attribute("pinned");
   if(pinnedOnly && !pinned)continue;
   var counts=progressAttributes.Select(k=>(int)rows[index].Attribute(k)).ToArray();
   int total=counts.Sum();
   string path=projects[index],name=ProjectName(path);
   if(IsCompany(path)){if(total==0)continue;name+="（直下）";}
   if(String.IsNullOrEmpty(name))name=path;
   var content=new DockPanel{Width=154};
   content.Children.Add(StatusPie(progressOrder.Select(i=>counts[i]).ToArray(),progressOrder.Select(i=>progressColors[i]).ToArray()));
   var caption=new StackPanel{Margin=new Thickness(7,0,0,0),VerticalAlignment=VerticalAlignment.Center};
   caption.Children.Add(Label(name,14));
   caption.Children.Add(Label(total+" 件",13));
   content.Children.Add(caption);
   string detail=name+"　"+String.Join("　",progressOrder.Select(i=>progressStates[i]+" "+counts[i]+"件"));
   var button=new Button{Style=buttonStyle,Content=content,ToolTip=detail+"\n右クリックで状態推移を表示",
    Padding=new Thickness(3,0,0,0),HorizontalContentAlignment=HorizontalAlignment.Left};
   AutomationProperties.SetName(button,detail);
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
   var actions=new Grid();
   actions.Children.Add(button);
   string hint=name+(pinned?" のピン留めを解除":" をピン留め");
   var pin=new Button{Style=buttonStyle,Content=pinned?"★":"☆",FontSize=18,Width=32,Height=26,ToolTip=hint,
    Margin=new Thickness(0,0,2,0),HorizontalAlignment=HorizontalAlignment.Right,VerticalAlignment=VerticalAlignment.Top};
   AutomationProperties.SetName(pin,hint);
   pin.Click+=(sender,args)=>Run(712,projectIndex);
   actions.Children.Add(pin);
   actions.Children.Add(new Border{Width=1,Height=24,Background=Color("#F6FBFC"),IsHitTestVisible=false,
    Margin=new Thickness(0,0,34,0),HorizontalAlignment=HorizontalAlignment.Right,VerticalAlignment=VerticalAlignment.Top});
   chart.Children.Add(new Border{Child=actions,Width=196,Height=47,CornerRadius=new CornerRadius(5),
    Background=Color(index==selected?"#D1E8E7":"#E3EEF0"),Margin=new Thickness(1,2,5,2)});
  }
  if(chart.Children.Count==0)chart.Children.Add(Label(pinnedOnly?"ピン留めしたプロジェクトはありません。「すべて表示」で ☆ を選んでください。":"プロジェクトがありません",14));
 }
 readonly Dictionary<string,string> projectAliases=new Dictionary<string,string>(StringComparer.OrdinalIgnoreCase);
 string ProjectName(string path){
  string alias;
  return projectAliases.TryGetValue(path,out alias)?alias:Path.GetFileName(path.TrimEnd(Path.DirectorySeparatorChar));
 }
 static bool IsGuardProject(string path){
  var parent=Path.GetDirectoryName(path);
  if(parent==null || !String.Equals(Path.GetFileName(path.TrimEnd(Path.DirectorySeparatorChar)),"SHIRANUI",StringComparison.OrdinalIgnoreCase))return false;
  // 2026-11-25: SHIRANUI moved from .company\SHIRANUI to .company\os\shiranui.
  if(IsCompany(parent))return true;
  var grand=Path.GetDirectoryName(parent);
  return grand!=null && IsCompany(grand) && String.Equals(Path.GetFileName(parent),"os",StringComparison.OrdinalIgnoreCase);
 }
 TreeViewItem ProjectNode(string path,int index){
  var name=ProjectName(path);
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
  if(!IsCompany(path)){
   var diff=new MenuItem{Header="Git差分"};
   diff.Click+=(sender,args)=>{
    args.Handled=true;
    node.IsSelected=true;
    if((int)data.Attribute("project")!=index && !Run(701,index))return;
    if((flags&8)!=0 && !Run(197))return;
    Run(121);
   };
   menu.Items.Add(diff);
   var map=new MenuItem{Header="ドキュメント・モジュール関連図"};
   map.Click+=(sender,args)=>{args.Handled=true;ShowProjectMap(path,index);};
   menu.Items.Add(map);
  }
  menu.Items.Add(startup);
  var routing=new MenuItem{Header="外部連絡の取込設定…"};
  routing.Click+=(s,e)=>{e.Handled=true;MessageRouting.Open(window,PathName(path),data.Elements("project").Select(p=>PathName(p.Value)).ToArray());};
  menu.Items.Add(routing);
  var projectFolder=Path.GetFileName(path);
  bool politics=String.Equals(projectFolder,"politics",StringComparison.OrdinalIgnoreCase);
  bool eda=String.Equals(projectFolder,"eda",StringComparison.OrdinalIgnoreCase);
  if(IsCompany(Path.GetDirectoryName(path)) && (eda || politics || String.Equals(projectFolder,"tekisei_quest",StringComparison.OrdinalIgnoreCase))){
   string directory=PathName(path);
   if(eda)directory=Path.Combine(directory,"app");
   if(politics)directory=Path.Combine(directory,@"ayumi\app");
   string cli=eda?@"node_modules\.bin\wrangler.cmd":"npx.cmd --yes wrangler@4.160.0";
   string scopes="account:read user:read pages:write"+(eda?" workers_scripts:write":"");
   string verify=eda?"deployments list --name eda":"pages deployment list --environment production --project-name "+(politics?"ayumi-tsushinbo":"tekisei-quest");
   var auth=new MenuItem{Header="Cloudflare の公開権限を登録…"};
   auth.Click+=(sender,args)=>{
    args.Handled=true;
    try{
     var command=new System.Diagnostics.ProcessStartInfo{
      FileName=Path.Combine(Environment.SystemDirectory,"cmd.exe"),
      Arguments="/d /k "+cli+" login --browser=true --scopes "+scopes+" && "+cli+" "+verify,
      WorkingDirectory=directory,UseShellExecute=true
     };
     using(var process=System.Diagnostics.Process.Start(command)){}
    }catch(Exception ex){MessageBox.Show(window,ex.Message,"Cloudflare の認証");}
   };
   menu.Items.Add(auth);
  }
  if(IsGuardProject(path)){var guard=new MenuItem{Header="監視・アラート"};guard.Click+=(s,e)=>{e.Handled=true;ShowGuard();};menu.Items.Add(guard);}
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
  PreviewExplorerEntry(entry);
 }
 void PreviewExplorerEntry(FileSystemInfo entry){
  CancelTaskDocumentSearch();
  At<Expander>("DocumentOutline").Visibility=Visibility.Collapsed;At<TextBlock>("CurrentRelation").Visibility=Visibility.Collapsed;SetDocumentTab(null);
  At<StackPanel>("DocumentContext").Visibility=Visibility.Collapsed;
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
 At<Border>("Kpi").Visibility=Visible((f&2)==0);var panes=At<Grid>("Panes");var docs=At<Border>("Docs");var grip=At<GridSplitter>("Grip");At<ScrollViewer>("QueueColumn").Visibility=Visible(q);docs.Visibility=Visible(d);grip.Visibility=Visible(both);
 var zero=new GridLength(0);var star=new GridLength(1,GridUnitType.Star);panes.ColumnDefinitions[0].Width=star;panes.RowDefinitions[0].Height=star;panes.ColumnDefinitions[1].Width=both&&!vert?new GridLength(6):zero;panes.ColumnDefinitions[2].Width=both&&!vert?new GridLength(2,GridUnitType.Star):zero;panes.RowDefinitions[1].Height=vert?new GridLength(6):zero;panes.RowDefinitions[2].Height=vert?star:zero;
 Grid.SetColumn(docs,both&&!vert?2:0);Grid.SetRow(docs,vert?2:0);Grid.SetColumn(grip,both&&!vert?1:0);Grid.SetRow(grip,vert?1:0);grip.ResizeDirection=vert?GridResizeDirection.Rows:GridResizeDirection.Columns;
 }
 [STAThread] static int Main(string[] args){
 if(args.Length==1 && args[0]=="--jobs-patrol")return RunJobsPatrol();
 if(args.Length==2 && args[0]=="--news-receive")return System.Threading.Tasks.Task.Run(()=>ReceiveScheduledDots(args[1])).GetAwaiter().GetResult();
 if(args.Length==2 && args[0]=="--news-patrol")return System.Threading.Tasks.Task.Run(()=>RunNewsPatrol(args[1])).GetAwaiter().GetResult();
 bool alertClick=args.Length>0 && args[0]=="--open-alert";
 bool notification=alertClick || (args.Length>0 && args[0]=="--open-task");
 if(notification){
  if(args.Length!=2)return 2;
  var match=(alertClick?guardNotificationUri:notificationUri).Match(args[1]);
  if(!match.Success)return 2;
  QueueClick((alertClick?"alert-":"")+match.Groups[1].Value);
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
 var tile=ui.At<WrapPanel>("Chart").Children.OfType<Border>().FirstOrDefault();
 if(tile!=null){
  var pie=((Grid)tile.Child).Children.OfType<Button>().First();
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
  var projectNode=ui.projectNodes[fixture];
  projectNode.IsExpanded=true;ui.window.UpdateLayout();
  var fileNode=projectNode.Items.OfType<TreeViewItem>().Single(n=>String.Equals(n.ToolTip as string,Path.Combine(testRoot,"result.md"),StringComparison.OrdinalIgnoreCase));
  var nodes=projectNode.Items.OfType<TreeViewItem>().Where(n=>n.Tag is DirectoryInfo).Concat(new[]{projectNode,fileNode});
  foreach(var node in nodes){
   var icon=((StackPanel)node.Header).Children[0] as Border;
   if(icon==null || !(icon.OpacityMask is ImageBrush))throw new Exception("エクスプローラーの画像アイコン");
  }
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
