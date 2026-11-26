using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;

using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

internal sealed partial class Desktop {
 static string GuardLocalFolder(){var folder=Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);return String.IsNullOrEmpty(folder)?Environment.GetEnvironmentVariable("LOCALAPPDATA"):folder;}
 static string AlertValue(object obj,string field){return Convert.ToString(Field(obj,field),CultureInfo.InvariantCulture)??"";}
 static object[] AlertArray(object obj,string field){var a=Field(obj,field) as IEnumerable;return a==null?new object[0]:a.Cast<object>().ToArray();}
 static string AlertTime(object obj,string field){
  var t=ParseStamp(Field(obj,field));return t==DateTime.MinValue?"未取得":t.ToLocalTime().ToString("yyyy/MM/dd HH:mm:ss");
 }
 static string AlertLevel(object row){
  switch(AlertValue(row,"severity")){case "critical":return "緊急";case "high":return "高";case "medium":return "注意";case "low":return "低";default:return "未取得";}
 }
 static string ProcessName(object row){
  var path=AlertValue(row,"image").Replace('/','\\');var name=path.Split('\\').Last();return name.Length>0?name:"未取得";
 }
 static void AlertHeading(Panel panel,string text){
  var heading=Label(text,16);heading.Margin=new Thickness(0,14,0,4);heading.FontWeight=FontWeights.SemiBold;panel.Children.Add(heading);
 }
 static void RenderAlert(StackPanel panel,object row){
  panel.Children.Clear();if(row==null)return;
  var title=Label(ProcessName(row),21);title.FontWeight=FontWeights.SemiBold;panel.Children.Add(title);
  panel.Children.Add(UsageText("重要度　"+AlertLevel(row)+"　　検知スコア　"+GuardValue(row,"score")+"　　発生回数　"+GuardValue(row,"occurrences")));
  panel.Children.Add(UsageText("ルール　"+GuardValue(row,"rule")+"　　対応状態　"+ReviewState(row)+"　　アラートID　"+GuardValue(row,"id")));
  panel.Children.Add(UsageText("検知スコアが高いほど優先。SHIRANUIの基準は50点から注意、75点から高、90点から緊急。"));
  AlertHeading(panel,"検知理由");
  panel.Children.Add(UsageText(GuardValue(row,"reason")));
  panel.Children.Add(UsageText("初回　"+AlertTime(row,"first_utc")+"　　最終　"+AlertTime(row,"last_utc")));
  AlertHeading(panel,"実行ファイル");
  var file=new TextBox{Text=GuardValue(row,"image"),IsReadOnly=true,TextWrapping=TextWrapping.Wrap};
  var line=new DockPanel();var copy=new Button{Content="パスをコピー",IsEnabled=AlertValue(row,"image").Length>0};
  copy.Click+=(s,e)=>{try{Clipboard.SetText(file.Text);}catch(System.Runtime.InteropServices.COMException){file.Focus();file.SelectAll();}};
  DockPanel.SetDock(copy,Dock.Right);line.Children.Add(copy);line.Children.Add(file);panel.Children.Add(line);
  panel.Children.Add(new Expander{Header="実行情報",Content=UsageText("実行ID　"+GuardValue(row,"process_guid")+"　　PID　"+GuardValue(row,"pid")+"　　端末　"+GuardValue(row,"host")+"\nコマンド　"+GuardValue(row,"commandLine")+"\n署名　"+GuardValue(row,"signer")+"　　発行元　"+GuardValue(row,"publisher")+"\nSHA-256　"+GuardValue(row,"hashSha256"))});
  string target=AlertValue(row,"target");
  if(target.Length>0){AlertHeading(panel,"検知対象");panel.Children.Add(UsageText(target));}
  var path=Field(row,"path");var nodes=AlertArray(path,"nodes");var events=AlertArray(path,"events");
  if(nodes.Length==0){panel.Children.Add(UsageText("プロセス起動経路は未取得です。"));return;}
  var open=new Button{Content="プロセス起動経路を開く",HorizontalAlignment=HorizontalAlignment.Left,Margin=new Thickness(0,12,0,4)};
  open.Click+=(s,e)=>{
   var owner=Window.GetWindow(open);
   var dialog=new Window{Title="プロセス起動経路（記録された起動順） — "+ProcessName(row),Owner=owner,
    Width=Math.Min(1120,owner.ActualWidth),Height=Math.Min(720,owner.ActualHeight),
    WindowStartupLocation=WindowStartupLocation.CenterOwner,ShowInTaskbar=false};
   var content=new DockPanel{Margin=new Thickness(12)};
   var footer=new DockPanel{LastChildFill=false};
   var close=new Button{Content="閉じる",IsCancel=true,MinWidth=80};
   DockPanel.SetDock(close,Dock.Right);footer.Children.Add(close);
   if(Equals(Field(path,"truncated"),true))footer.Children.Add(UsageText("経路の一部は記録範囲外です。"));
   DockPanel.SetDock(footer,Dock.Bottom);content.Children.Add(footer);
   content.Children.Add(TraceGraph(row,path,nodes,events));
   dialog.Content=content;dialog.ShowDialog();
  };
  panel.Children.Add(open);
 }
 Button guardButton;
 double? workspaceMinimumWidth;
 void HideGuard(){
  if(workspaceMinimumWidth.HasValue){window.MinWidth=workspaceMinimumWidth.Value;workspaceMinimumWidth=null;}
  At<Border>("Guard").Visibility=Visibility.Collapsed;
  System.Windows.Controls.Primitives.Selector.SetIsSelected(guardButton,false);
 }
 void ShowGuard(string alertId=null){
  HideNews();
  HideCulture();
  if(!workspaceMinimumWidth.HasValue)workspaceMinimumWidth=window.MinWidth;
  window.MinWidth=Math.Min(workspaceMinimumWidth.Value,640);
  foreach(var b in At<StackPanel>("Rail").Children.OfType<Button>())
   System.Windows.Controls.Primitives.Selector.SetIsSelected(b,b==guardButton);
  At<Grid>("Workspace").Visibility=Visibility.Collapsed;
  At<Border>("Analytics").Visibility=Visibility.Collapsed;
  var host=At<Border>("Guard");
  var root=new DockPanel();
  var banner=new DockPanel{LastChildFill=true,Margin=new Thickness(0,0,0,6)};banner.SetResourceReference(Panel.BackgroundProperty,"WorkspaceCanvas");
  DockPanel.SetDock(banner,Dock.Top);root.Children.Add(banner);
  var title=Label("SHIRANUI",22);title.Margin=new Thickness(16,7,18,7);title.SetResourceReference(TextBlock.ForegroundProperty,"ActionPrimary");banner.Children.Add(title);
  var services=Label("",16);services.TextWrapping=TextWrapping.Wrap;services.SetResourceReference(TextBlock.ForegroundProperty,"TextPrimary");services.VerticalAlignment=VerticalAlignment.Center;banner.Children.Add(services);
  services.ToolTip="Windows サービスの現在状態。Sysmon の設定・ハートビートは未取得。";
  Action refreshServices=()=>{services.Text=GuardService("LocalGuardAI")+"　　　"+GuardService("Sysmon64");};
  var timer=new System.Windows.Threading.DispatcherTimer{Interval=TimeSpan.FromSeconds(5)};
  timer.Tick+=(s,e)=>refreshServices();
  root.IsVisibleChanged+=(s,e)=>{if(root.IsVisible){refreshServices();timer.Start();}else timer.Stop();};
  var dataDir=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData),"LocalGuardAI");
  var history=Path.Combine(GuardLocalFolder(),"LocalGuardAI","alerts.html");
  root.Children.Add(GuardDashboard(Path.Combine(dataDir,"alert-feed.json"),history,alertId));
  host.Child=root;host.Visibility=Visibility.Visible;
 }
 static string VerdictLabel(string verdict){
  switch(verdict){case "benign":return "問題なし";case "suspicious":return "要確認";default:return "未判断";}
 }
 static void SubmitVerdict(string folder,long id,string verdict){
  string name=DateTime.UtcNow.ToString("yyyyMMddHHmmssfff",CultureInfo.InvariantCulture)+"-"+Guid.NewGuid().ToString("N");
  string temp=Path.Combine(folder,name+".tmp");
  var json=new JavaScriptSerializer().Serialize(new {alertId=id,verdict=verdict,utc=DateTime.UtcNow.ToString("o")});
  Directory.CreateDirectory(folder);
  try{File.WriteAllText(temp,json);File.Move(temp,Path.ChangeExtension(temp,".json"));}
  finally{if(File.Exists(temp))File.Delete(temp);}
 }
}
