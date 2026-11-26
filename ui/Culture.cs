using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Media.Imaging;
using System.Windows.Threading;

internal sealed partial class Desktop {
 Button cultureButton;
 readonly RadioButton[] cultureChoices=new RadioButton[2];
 readonly Image[] cultureImages=new Image[2];
 readonly List<BitmapSource>[] cultureFrames={new List<BitmapSource>(),new List<BitmapSource>()};
 DispatcherTimer cultureMotion;
 int cultureIndex,cultureFrame;
 static string ModelRoot(int i){
  string relative=i==0?@"mebae\ClaudeCode\mebae-l":@"dots\mebae-c";
  for(var parent=new DirectoryInfo(folder).Parent;parent!=null;parent=parent.Parent){
   string candidate=Path.Combine(parent.FullName,relative);
   if(Directory.Exists(candidate))return candidate;
  }
  return Path.GetFullPath(Path.Combine(folder,"..",relative));
 }
 static string ModelName(int i){return i==0?"MEBAE-L":"MEBAE-C";}
 static string ModelExe(int i){
  string name=i==0?"mebae-desktop.exe":"mebae-c-desktop.exe";
  var paths=new[]{"release","debug"}.Select(mode=>Path.Combine(ModelRoot(i),"target",mode,name));
  return paths.FirstOrDefault(File.Exists)??paths.First();
 }
 static BitmapSource ModelBitmap(string path){
  using(var stream=File.OpenRead(path)){
   var image=new BitmapImage();image.BeginInit();image.CacheOption=BitmapCacheOption.OnLoad;
   image.StreamSource=stream;image.EndInit();image.Freeze();return image;
  }
 }
 void LoadModelFrames(int i){
  var frames=cultureFrames[i];if(frames.Count>0)return;
  try{
   if(i==0){for(int n=1;n<=8;n++)frames.Add(ModelBitmap(Path.Combine(ModelRoot(i),@"apps\desktop\src\assets","l-walkr-"+n+".png")));}
   else{
    var atlas=ModelBitmap(Path.Combine(ModelRoot(i),@"apps\dashboard\public\mascot-c-motion-v1.png"));
    for(int n=2;n<=5;n++){
     int col=n%4,row=n/4,x=col*atlas.PixelWidth/4,y=row*atlas.PixelHeight/4;
     var frame=new CroppedBitmap(atlas,new Int32Rect(x,y,(col+1)*atlas.PixelWidth/4-x,(row+1)*atlas.PixelHeight/4-y));
     frame.Freeze();frames.Add(frame);
    }
   }
  }catch(IOException){frames.Clear();}catch(NotSupportedException){frames.Clear();}
 }
 void OpenCulture(){
  HideNews();
  HideGuard();
  if(cultureMotion==null){
   for(int i=0;i<2;i++){
    int index=i;
    var card=new RadioButton{GroupName="ModelCulture",Padding=new Thickness(12),Margin=new Thickness(0,0,14,0),VerticalContentAlignment=VerticalAlignment.Center};
    AutomationProperties.SetName(card,ModelName(i));
    var stack=new StackPanel{Width=82};cultureImages[i]=new Image{Height=50};
    stack.Children.Add(cultureImages[i]);stack.Children.Add(new TextBlock{Text=ModelName(i),FontSize=15,HorizontalAlignment=HorizontalAlignment.Center});
    card.Content=stack;card.Checked+=(s,e)=>{cultureIndex=index;RefreshModel();ShowManager(false);};cultureChoices[i]=card;
    At<UniformGrid>("CultureCards").Children.Add(card);
   }
   At<Button>("CultureRefresh").Click+=(s,e)=>{RefreshModel();ShowManager(true);};
   window.Closing+=(s,e)=>ReleaseManager();
   cultureMotion=new DispatcherTimer{Interval=TimeSpan.FromMilliseconds(150)};
   cultureMotion.Tick+=(s,e)=>{cultureFrame=(cultureFrame+1)%8;DrawModels();};
   window.Closed+=(s,e)=>cultureMotion.Stop();
  }
  foreach(var b in At<StackPanel>("Rail").Children.OfType<Button>())Selector.SetIsSelected(b,b==cultureButton);
  At<Grid>("Workspace").Visibility=Visibility.Collapsed;At<Border>("Analytics").Visibility=Visibility.Collapsed;
  At<Border>("Culture").Visibility=Visibility.Visible;
  if(cultureChoices[cultureIndex].IsChecked==true){RefreshModel();ShowManager(false);}else cultureChoices[cultureIndex].IsChecked=true;
  if(SystemParameters.ClientAreaAnimation)cultureMotion.Start();
 }
 void HideCulture(){
  if(managerIndex>=0)ReleaseManager();
  At<Border>("Culture").Visibility=Visibility.Collapsed;
  if(cultureButton!=null)Selector.SetIsSelected(cultureButton,false);
  if(cultureMotion!=null)cultureMotion.Stop();
 }
 void DrawModels(){for(int i=0;i<2;i++)if(cultureFrames[i].Count>0)cultureImages[i].Source=cultureFrames[i][cultureFrame%cultureFrames[i].Count];}
 static Dictionary<string,object> ModelJson(string path){
  return (Dictionary<string,object>)new JavaScriptSerializer().DeserializeObject(File.ReadAllText(path));
 }
 static string CutoverPath(){return Path.Combine(ModelRoot(1),@"data\codex\cloud-cutover.json");}
 void RefreshModel(){
  int i=cultureIndex;string root=ModelRoot(i);
  LoadModelFrames(0);LoadModelFrames(1);DrawModels();
  bool cloud=i==1 && File.Exists(CutoverPath());
  At<TextBlock>("CultureTitle").Text=ModelName(i)+" の管理";
  At<TextBlock>("CultureInfo").Text=(cloud?"クラウド運用":"ローカル運用")+
   (cultureFrames[i].Count==0?"\nアバター素材を読み込めません。":"");
  string path=Path.Combine(root,i==0?@"reports\blind_evaluation_latest.json":@"reports\resume-20261108.json");
  string report;
  try{
   var doc=ModelJson(path);
   report=i==0?"評価対象　"+((object[])doc["agents"]).Length+" 個体　／　"+((object[])doc["experiments"]).Length+" 実験":
    "復元検証　"+doc["restored_experiments_visible"]+" 実験　／　バックアップ整合性　"+doc["backup_integrity"];
   report+="\n記録　"+Path.GetFileName(path)+"　"+File.GetLastWriteTime(path).ToString("yyyy/MM/dd HH:mm");
  }catch(Exception){report="検証記録を読み込めません。";}
  At<TextBlock>("CultureReport").Text=report;

 }
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern IntPtr FindWindow(string cls,string title);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 static IntPtr ManagerWindow(int i,string exe){
  var h=FindWindow(null,i==0?"MEBAE-L 個体管理":"MEBAE-C · 個体管理");uint pid;
  if(h==IntPtr.Zero || GetWindowThreadProcessId(h,out pid)==0)return IntPtr.Zero;
  using(var p=Process.GetProcessById((int)pid))
   return string.Equals(p.MainModule.FileName,exe,StringComparison.OrdinalIgnoreCase)?h:IntPtr.Zero;
 }
 int managerIndex=-1, managerGeneration;
 NativeManagerHost nativeManager;
 Microsoft.Web.WebView2.Wpf.WebView2 cloudManager;
 void ManagerMessage(string message){
  var label=At<TextBlock>("ManagerStatus");label.Text=message;label.Visibility=message.Length==0?Visibility.Collapsed:Visibility.Visible;
 }
 void ReleaseManager(){
  managerGeneration++;managerIndex=-1;
  if(nativeManager!=null){nativeManager.Dispose();nativeManager=null;}
  At<ContentControl>("ManagerArea").Content=null;
  if(cloudManager!=null){cloudManager.Dispose();cloudManager=null;}
 }
 async void ShowManager(bool refresh){
  int i=cultureIndex;
  if(!refresh && managerIndex==i)return;
  ReleaseManager();int generation=managerGeneration;managerIndex=i;
  ManagerMessage("個体管理を読み込んでいます…");
  try{
   var area=At<ContentControl>("ManagerArea");
   if(i==1 && File.Exists(CutoverPath())){
    Uri url;
    if(!Uri.TryCreate(Convert.ToString(ModelJson(CutoverPath())["url"]),UriKind.Absolute,out url)||url.Scheme!="https")throw new InvalidDataException("クラウドの接続先を確認してください。");
    var web=new Microsoft.Web.WebView2.Wpf.WebView2();
    cloudManager=web;
    web.CreationProperties=new Microsoft.Web.WebView2.Wpf.CoreWebView2CreationProperties{UserDataFolder=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"RustAIWorkspace","mebae-c-web")};
    var pane=new DockPanel();
    var sections=new ListBox{Width=185,Margin=new Thickness(0,0,10,0),Visibility=Visibility.Collapsed};
    AutomationProperties.SetName(sections,"観測室の項目");
    DockPanel.SetDock(sections,Dock.Left);pane.Children.Add(sections);
    var content=new Grid();pane.Children.Add(content);content.Children.Add(web);
    var login=new StackPanel{Width=340,HorizontalAlignment=HorizontalAlignment.Center,VerticalAlignment=VerticalAlignment.Center,Visibility=Visibility.Collapsed};
    login.Children.Add(Label("MEBAE-C 観測室",21));
    var key=new PasswordBox{MaxLength=128,Padding=new Thickness(8,5,8,5)};
    login.Children.Add(new Label{Content="アクセスキー",Target=key});login.Children.Add(key);
    var enter=new Button{Content="観測室を開く",Style=(Style)window.FindResource("PrimaryAction"),Margin=new Thickness(0,10,0,0)};
    login.Children.Add(enter);content.Children.Add(login);area.Content=pane;
    await web.EnsureCoreWebView2Async();
    if(generation!=managerGeneration)return;
    var engine=web.CoreWebView2;
    string origin=url.GetLeftPart(UriPartial.Authority);
    engine.NavigationStarting+=(s,e)=>{Uri next;e.Cancel=!Uri.TryCreate(e.Uri,UriKind.Absolute,out next)||next.GetLeftPart(UriPartial.Authority)!=origin;};
    engine.NewWindowRequested+=(s,e)=>{e.Handled=true;Uri next;if(Uri.TryCreate(e.Uri,UriKind.Absolute,out next)&&next.GetLeftPart(UriPartial.Authority)==origin)web.Source=next;};
    enter.Click+=async (s,e)=>{
     if(key.Password.Length==0){key.Focus();return;}
     var json=new JavaScriptSerializer();
     string submit="(()=>{if(location.origin!=="+json.Serialize(origin)+"||location.pathname!=='/login')return false;"+
      "const f=document.querySelector('form[action=\"/login\"][method=\"post\"]'),p=f&&f.querySelector('input[type=password][name=password]');"+
      "if(!p)return false;p.value="+json.Serialize(key.Password)+";f.requestSubmit();return true})()";
     key.Clear();enter.IsEnabled=false;ManagerMessage("接続しています…");
     try{if(await engine.ExecuteScriptAsync(submit)=="true")return;}
     catch(InvalidOperationException){}
     catch(COMException){}
     if(generation==managerGeneration){enter.IsEnabled=true;ManagerMessage("認証画面を読み込めません。［更新］で再試行してください。");}
    };
    key.KeyDown+=(s,e)=>{if(e.Key==System.Windows.Input.Key.Enter && enter.IsEnabled){e.Handled=true;enter.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));}};
    engine.WebMessageReceived+=(s,e)=>{
     Uri from;if(generation!=managerGeneration || !Uri.TryCreate(e.Source,UriKind.Absolute,out from) || from.GetLeftPart(UriPartial.Authority)!=origin || login.IsVisible)return;
     string[] names;
     try{names=new JavaScriptSerializer().Deserialize<string[]>(e.WebMessageAsJson);}catch(ArgumentException){return;}catch(InvalidOperationException){return;}
     if(names==null)return;
     sections.ItemsSource=names;sections.Visibility=names.Length>0?Visibility.Visible:Visibility.Collapsed;
    };
    sections.SelectionChanged+=(s,e)=>{int index=sections.SelectedIndex;if(index>=0)engine.ExecuteScriptAsync("document.querySelectorAll('main h2,main h3')["+index+"]?.scrollIntoView({block:'start'})");};
    web.NavigationCompleted+=async (s,e)=>{
     if(generation!=managerGeneration)return;
     enter.IsEnabled=true;
     bool atLogin=web.Source!=null && web.Source.AbsolutePath=="/login";
     login.Visibility=atLogin?Visibility.Visible:Visibility.Collapsed;
     web.Visibility=atLogin?Visibility.Hidden:Visibility.Visible;
     sections.Visibility=Visibility.Collapsed;
     ManagerMessage(e.HttpStatusCode==401?"アクセスキーを確認してください。":e.HttpStatusCode==429?"しばらく待ってから再試行してください。":!e.IsSuccess?"接続できません。［更新］で再接続できます。":"");
     if(atLogin){key.Focus();return;}
     if(e.IsSuccess){
      try{await engine.ExecuteScriptAsync(@"(()=>{
       if(window!==top || window.kagariSections)return;
       let last=null;function update(){let names=Array.from(document.querySelectorAll('main h2,main h3'),h=>h.textContent.trim());let text=JSON.stringify(names);if(text!==last){last=text;chrome.webview.postMessage(names);}}
       let timer;window.kagariSections=new MutationObserver(()=>{clearTimeout(timer);timer=setTimeout(update,120)});
       window.kagariSections.observe(document.body,{childList:true,subtree:true,characterData:true});update();
      })()");}catch(InvalidOperationException){if(generation==managerGeneration)ManagerMessage("［更新］で再接続してください。");}
     }
    };
    web.Source=url;return;
   }
   string exe=ModelExe(i);var h=ManagerWindow(i,exe);
   if(h==IntPtr.Zero){
    bool running=false;
    foreach(var p in Process.GetProcessesByName(Path.GetFileNameWithoutExtension(exe)))using(p){if(string.Equals(p.MainModule.FileName,exe,StringComparison.OrdinalIgnoreCase))running=true;}
    if(!running)using(Process.Start(new ProcessStartInfo(exe){WorkingDirectory=ModelRoot(i),UseShellExecute=false})){}
    for(int n=0;n<80 && h==IntPtr.Zero;n++){await Task.Delay(250);if(generation!=managerGeneration)return;h=ManagerWindow(i,exe);}
   }
   if(h==IntPtr.Zero)throw new IOException("個体管理を起動できませんでした。");
   nativeManager=new NativeManagerHost();
   area.Content=nativeManager;
   area.UpdateLayout();
   nativeManager.Attach(h);
   ManagerMessage("");
  }catch(Exception ex){if(generation==managerGeneration){ReleaseManager();ManagerMessage(ex.Message+"　［更新］で再試行できます。");}}
 }
}
