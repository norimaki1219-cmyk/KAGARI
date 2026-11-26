using System;
using System.Collections;
using System.Collections.Generic;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Linq;
using System.ServiceProcess;
using System.Text.RegularExpressions;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Media;

internal sealed partial class Desktop {
 const string UnknownGuard="未取得";
 static string GuardValue(object row,string key){
  string value=AlertValue(row,key);return String.IsNullOrWhiteSpace(value)?UnknownGuard:value;
 }
 static string GuardService(string name){
  try{using(var service=new ServiceController(name)){
   switch(service.Status){
    case ServiceControllerStatus.Running:return name+"　稼働中";
    case ServiceControllerStatus.Stopped:return name+"　停止中";
    case ServiceControllerStatus.Paused:return name+"　一時停止";
    default:return name+"　移行中 ("+service.Status+")";
   }
  }}catch(InvalidOperationException ex){
   var error=ex.InnerException as Win32Exception;
   return name+(error!=null && error.NativeErrorCode==1060?"　未インストール":"　未取得");
  }catch(Win32Exception){return name+"　未取得";}catch(System.Security.SecurityException){return name+"　未取得";}
 }
 static string ReviewState(object row){
  var data=row as IDictionary<string,object>;
  if(data==null || !data.ContainsKey("verdict"))return UnknownGuard;
  switch(AlertValue(row,"verdict")){
   case "":case "clear":return "未対応";
   case "benign":return "判断済・問題なし";
   case "suspicious":return "判断済・要確認";
   default:return UnknownGuard;
  }
 }
 sealed class GuardRow {
  public object Raw;
  public string Id {get{return GuardValue(Raw,"id");}}
  public string Name {get{return ProcessName(Raw);}}
  public string Rule {get{return GuardValue(Raw,"rule");}}
  public string Level {get{return AlertLevel(Raw);}}
  public string Review {get{return ReviewState(Raw);}}
  public string Count {get{return GuardValue(Raw,"occurrences");}}
  public string Time {get{return AlertTime(Raw,"last_utc");}}
  public DateTime Last {get{return ParseStamp(Field(Raw,"last_utc"));}}
  public int Rank {get{return Array.IndexOf(new[]{"low","medium","high","critical"},AlertValue(Raw,"severity"));}}
 }
 static object ReadGuardSnapshot(string path,bool html){
  string json;using(var reader=new StreamReader(OpenUsage(path)))json=reader.ReadToEnd();
  if(html){
   var match=Regex.Match(json,"<script\\s+id=\"alert-data\"\\s+type=\"application/json\"\\s*>(.*?)</script>",RegexOptions.Singleline|RegexOptions.IgnoreCase);
   if(!match.Success)throw new InvalidDataException();json=match.Groups[1].Value;
  }
  var snapshot=new JavaScriptSerializer{MaxJsonLength=Int32.MaxValue}.DeserializeObject(json);
  if(!(Field(snapshot,"alerts") is IList) || AlertArray(snapshot,"alerts").Any(x=>!(x is IDictionary<string,object>)))throw new InvalidDataException();
  return snapshot;
 }
 static bool GuardReadFailure(Exception e){
  return e is IOException || e is InvalidDataException || e is UnauthorizedAccessException || e is InvalidOperationException || e is ArgumentException || e is FormatException;
 }
 // Geometry from the package's activity, filter, alert-triangle and chart SVGs.
 static Border GuardCard(string title,string geometry,out TextBlock value){
  var panel=new StackPanel();var caption=new StackPanel{Orientation=Orientation.Horizontal};
  caption.Children.Add(new System.Windows.Shapes.Path{Data=Geometry.Parse(geometry),Stretch=Stretch.Uniform,Width=17,Height=17,Stroke=Color("#08655F"),StrokeThickness=1.8,Margin=new Thickness(0,0,7,0)});
  var name=Label(title,13);name.SetResourceReference(TextBlock.ForegroundProperty,"TextSecondary");caption.Children.Add(name);panel.Children.Add(caption);
  value=Label(UnknownGuard,23);value.SetResourceReference(TextBlock.ForegroundProperty,"TextPrimary");panel.Children.Add(value);
  var card=new Border{Child=panel,BorderThickness=new Thickness(1),CornerRadius=new CornerRadius(7),Padding=new Thickness(12,6,8,6),Margin=new Thickness(2)};card.SetResourceReference(Border.BackgroundProperty,"Surface");card.SetResourceReference(Border.BorderBrushProperty,"Separator");return card;
 }
 static ComboBox GuardChoice(WrapPanel filters,string name,string[] options,int index){
  var box=new ComboBox{ItemsSource=options,SelectedIndex=index,Margin=new Thickness(3),ToolTip=name};
  AutomationProperties.SetName(box,name);filters.Children.Add(box);return box;
 }
 // Save only view options. Search text can contain paths or other evidence.
 static string GuardViewFile(){return Path.Combine(GuardLocalFolder(),"RustAIWorkspace","shiranui-view.txt");}
 void GuardPreferences(bool save){
  try{
   var path=GuardViewFile();
   if(save){
    Directory.CreateDirectory(Path.GetDirectoryName(path));
    var temp=path+"."+Guid.NewGuid().ToString("N");
    try{
     File.WriteAllLines(temp,new[]{guardLevel.ToString(),guardPeriod.ToString(),guardOrder.ToString(),guardPending.ToString()});
     if(File.Exists(path))File.Replace(temp,path,null);else File.Move(temp,path);
    }finally{if(File.Exists(temp))File.Delete(temp);}
   }else if(File.Exists(path)){
    var v=File.ReadAllLines(path);int l,p,o;bool pending;
    if(v.Length==4 && Int32.TryParse(v[0],out l) && l>=0 && l<=6 && Int32.TryParse(v[1],out p) && p>=0 && p<=2 &&
     Int32.TryParse(v[2],out o) && o>=0 && o<=2 && Boolean.TryParse(v[3],out pending)){
     guardLevel=l;guardPeriod=p;guardOrder=o;guardPending=pending;
    }
   }
  }catch(IOException){}catch(UnauthorizedAccessException){}catch(System.Security.SecurityException){}
 }
 static void SetGuardChoices(ComboBox box,IEnumerable<string> choices,string caption){
  var previous=box.SelectedItem as string;
  box.ItemsSource=new[]{caption}.Concat(choices.Distinct().OrderBy(x=>x,StringComparer.OrdinalIgnoreCase)).ToArray();
  box.SelectedItem=previous;if(box.SelectedIndex<0)box.SelectedIndex=0;
 }
 int guardLevel,guardPeriod,guardOrder;
 bool guardPending;
 FrameworkElement GuardDashboard(string feed,string history,string alertId=null){
  GuardPreferences(false);
  var layout=new DockPanel{Margin=new Thickness(8)};
  var header=new DockPanel();DockPanel.SetDock(header,Dock.Top);layout.Children.Add(header);
  var update=new Button{Content="表示を更新"};DockPanel.SetDock(update,Dock.Right);header.Children.Add(update);
  var stamp=UsageText("");stamp.VerticalAlignment=VerticalAlignment.Center;header.Children.Add(stamp);
  var metrics=new System.Windows.Controls.Primitives.UniformGrid{Rows=1};
  metrics.SizeChanged+=(s,e)=>{int cardRows=e.NewSize.Width<740?2:1;if(metrics.Rows!=cardRows)metrics.Rows=cardRows;};
  DockPanel.SetDock(metrics,Dock.Top);layout.Children.Add(metrics);
  var values=new TextBlock[4];string[] names={"履歴アラート","未対応（未判断）","高優先度（高・緊急）","最終検知が過去24時間"};
  string[] paths={"M3 12h4l3-8 4 16 3-8h4","M3 4h18l-7 8v7l-4 2v-9Z","M10.3 3.4 1.9 18a2 2 0 0 0 1.7 3h16.8a2 2 0 0 0 1.7-3L13.7 3.4a2 2 0 0 0-3.4 0Z M12 9v4m0 4h.01","M4 20V12m5 8V6m5 14v-9m5 9V3"};
  var cards=new Button[4];
  for(int i=0;i<cards.Length;i++){
   var card=new Button{Content=GuardCard(names[i],paths[i],out values[i]),ToolTip=names[i]+"の一覧を表示"};
   card.SetResourceReference(FrameworkElement.StyleProperty,"ProjectCardButton");
   card.HorizontalContentAlignment=HorizontalAlignment.Stretch;
   AutomationProperties.SetName(card,card.ToolTip.ToString());cards[i]=card;metrics.Children.Add(card);
  }
  var tabs=new TabControl();layout.Children.Add(tabs);
  var work=new DockPanel();tabs.Items.Add(new TabItem{Header="アラート履歴・判断",Content=work});
  var statistics=new StackPanel{Margin=new Thickness(10)};
  tabs.Items.Add(new TabItem{Header="ログ・統計",Content=new ScrollViewer{Content=statistics,VerticalScrollBarVisibility=ScrollBarVisibility.Auto}});
  var filters=new WrapPanel{Margin=new Thickness(3)};DockPanel.SetDock(filters,Dock.Top);work.Children.Add(filters);
  var sourceChoice=GuardChoice(filters,"履歴の取得元",new[]{"最新フィード（自動）","ローカル履歴"},0);
  var search=new TextBox{Width=165,Margin=new Thickness(3),ToolTip="プロセス・パス・ルール・対象・理由・ID"};
  filters.Children.Add(new System.Windows.Controls.Label{Content="検索",Target=search});filters.Children.Add(search);
  var level=GuardChoice(filters,"重要度",new[]{"重要度すべて","低","注意","高","緊急","未取得","高・緊急"},guardLevel);
  var period=GuardChoice(filters,"期間（最終検知）",new[]{"全期間","過去24時間","過去7日"},guardPeriod);
  var order=GuardChoice(filters,"並び順",new[]{"未対応・重要度順","新しい順","古い順"},guardOrder);
  var pending=new CheckBox{Content="未対応のみ",VerticalAlignment=VerticalAlignment.Center,IsChecked=guardPending,Margin=new Thickness(4)};filters.Children.Add(pending);
  var process=GuardChoice(filters,"対象プロセス",new[]{"全プロセス"},0);
  var rule=GuardChoice(filters,"検知ルール",new[]{"全ルール"},0);
  var host=GuardChoice(filters,"端末",new[]{"全端末"},0);
  process.MaxWidth=185;rule.MaxWidth=185;host.MaxWidth=150;
  var count=UsageText("");DockPanel.SetDock(count,Dock.Top);work.Children.Add(count);
  var split=new Grid();work.Children.Add(split);
  split.RowDefinitions.Add(new RowDefinition{Height=new GridLength(0.45,GridUnitType.Star)});
  split.RowDefinitions.Add(new RowDefinition{Height=new GridLength(6)});
  split.RowDefinitions.Add(new RowDefinition{Height=new GridLength(0.55,GridUnitType.Star)});
  var list=new ListView{SelectionMode=SelectionMode.Single};AutomationProperties.SetName(list,"検知イベント一覧");
  var view=new GridView();list.View=view;split.Children.Add(list);
  string[] fields={"Time","Name","Rule","Level","Count","Review","Id"};
  string[] columns={"最終検知日時","プロセス","検知ルール","重要度","発生回数","対応状態","ID"};
  for(int i=0;i<fields.Length;i++)view.Columns.Add(new GridViewColumn{Header=columns[i],Width=new double[]{160,165,145,63,72,122,64}[i],DisplayMemberBinding=new Binding(fields[i])});
  var grip=new GridSplitter{Height=6,HorizontalAlignment=HorizontalAlignment.Stretch,ResizeDirection=GridResizeDirection.Rows};
  Grid.SetRow(grip,1);split.Children.Add(grip);
  var inspector=new DockPanel();
  var handoff=WithIcon(new Button{Content="作業キューで調査",HorizontalAlignment=HorizontalAlignment.Right,Margin=new Thickness(6),ToolTip="選択した検知の調査タスクを登録します。登録済みの場合はそのタスクを開きます。"},"\uE8A5");
  DockPanel.SetDock(handoff,Dock.Top);inspector.Children.Add(handoff);
  handoff.Click+=(s,e)=>{
   var selected=list.SelectedItem as GuardRow;if(selected==null)return;
   string evidence=selected.Time+"　"+selected.Level+"　"+selected.Rule+"\n"+GuardValue(selected.Raw,"reason")+"\n"+GuardValue(selected.Raw,"image");
   if(Run(764,0,selected.Id+"\n"+selected.Name.Replace("\r"," ").Replace("\n"," ")+"\n"+evidence)){ShowWorkspace();Run(160);}
  };
  var actions=new WrapPanel{Margin=new Thickness(4),IsEnabled=false};
  var outcome=UsageText("");
  DockPanel.SetDock(actions,Dock.Top);inspector.Children.Add(actions);
  DockPanel.SetDock(outcome,Dock.Top);inspector.Children.Add(outcome);
  bool liveFeed=false;
  foreach(string verdict in new[]{"benign","suspicious","clear"}){
   var button=new Button{Content=verdict=="clear"?"判断を取り消す":VerdictLabel(verdict)};
   button.Click+=(s,e)=>{
    var row=list.SelectedItem as GuardRow;long id;
    if(!liveFeed || row==null || !Int64.TryParse(row.Id,out id) || id<=0)return;
    try{SubmitVerdict(Path.Combine(Path.GetDirectoryName(feed),"feedback"),id,verdict);outcome.Text=row.Name+"　"+button.Content+"（反映待ち）";}
    catch(UnauthorizedAccessException){outcome.Text="判断の保存にはSHIRANUIの受付フォルダーへの書き込み権限が必要です。";}
    catch(IOException){outcome.Text="判断を保存できませんでした。受付フォルダーを確認してください。";}
   };
   actions.Children.Add(button);
  }
  var detail=new StackPanel{Margin=new Thickness(8)};var scroll=new ScrollViewer{Content=detail,VerticalScrollBarVisibility=ScrollBarVisibility.Auto};
  inspector.Children.Add(scroll);Grid.SetRow(inspector,2);split.Children.Add(inspector);
  bool wide=false;
  split.SizeChanged+=(s,e)=>{
   bool side=e.NewSize.Width>=1050;if(side==wide)return;wide=side;
   split.RowDefinitions.Clear();split.ColumnDefinitions.Clear();
   if(side){
    split.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(1.25,GridUnitType.Star)});
    split.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(6)});
    split.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(1,GridUnitType.Star)});
   }else{
    split.RowDefinitions.Add(new RowDefinition{Height=new GridLength(.45,GridUnitType.Star)});
    split.RowDefinitions.Add(new RowDefinition{Height=new GridLength(6)});
    split.RowDefinitions.Add(new RowDefinition{Height=new GridLength(.55,GridUnitType.Star)});
   }
   grip.Width=side?6:Double.NaN;grip.Height=side?Double.NaN:6;
   grip.ResizeDirection=side?GridResizeDirection.Columns:GridResizeDirection.Rows;
   Grid.SetRow(grip,side?0:1);Grid.SetColumn(grip,side?1:0);Grid.SetRow(inspector,side?0:2);Grid.SetColumn(inspector,side?2:0);
  };
  handoff.IsEnabled=false;
  list.SelectionChanged+=(s,e)=>{var row=list.SelectedItem as GuardRow;long selectedId;handoff.IsEnabled=row!=null && Int64.TryParse(row.Id,out selectedId) && selectedId>0;actions.IsEnabled=liveFeed && handoff.IsEnabled;outcome.Text=row!=null && !liveFeed?"ローカル履歴を表示中。判断の登録は最新フィードで行えます。":"";RenderAlert(detail,row==null?null:row.Raw);scroll.ScrollToHome();};
  GuardRow[] rows=null;DateTime now=DateTime.UtcNow;
  bool reading=false;
  Action filter=()=>{
   if(reading)return;
   bool changed=guardLevel!=level.SelectedIndex || guardOrder!=order.SelectedIndex || guardPeriod!=period.SelectedIndex || guardPending!=(pending.IsChecked==true);
   guardLevel=level.SelectedIndex;guardOrder=order.SelectedIndex;guardPeriod=period.SelectedIndex;guardPending=pending.IsChecked==true;
   if(changed)GuardPreferences(true);
   var selected=list.SelectedItem as GuardRow;
   var query=search.Text.Trim();var since=guardPeriod==1?now.AddDays(-1):now.AddDays(-7);
   var matches=(rows??new GuardRow[0]).Where(r=>(guardLevel==0 || (guardLevel==6?r.Rank>=2:r.Level==(string)level.SelectedItem)) &&
    (process.SelectedIndex<=0 || r.Name==(string)process.SelectedItem) && (rule.SelectedIndex<=0 || r.Rule==(string)rule.SelectedItem) &&
    (host.SelectedIndex<=0 || GuardValue(r.Raw,"host")==(string)host.SelectedItem) && (!guardPending || r.Review=="未対応") && (guardPeriod==0 || (r.Last>=since && r.Last<=now)) &&
    (query.Length==0 || new[]{r.Name,r.Rule,r.Id,AlertValue(r.Raw,"image"),AlertValue(r.Raw,"reason"),AlertValue(r.Raw,"target")}.Any(v=>v.IndexOf(query,StringComparison.OrdinalIgnoreCase)>=0)));
   if(alertId!=null)matches=(rows??new GuardRow[0]).Where(r=>r.Id==alertId);
   var sorted=guardOrder==0?matches.OrderByDescending(r=>r.Review=="未対応").ThenByDescending(r=>r.Rank).ThenByDescending(r=>r.Last):
    guardOrder==1?matches.OrderByDescending(r=>r.Last):matches.OrderBy(r=>r.Last);
   var visible=sorted.ToArray();list.ItemsSource=visible;
   list.SelectedItem=(selected==null || selected.Id==UnknownGuard?null:visible.FirstOrDefault(r=>r.Id==selected.Id))??visible.FirstOrDefault();
   count.Text=rows==null?"イベント一覧　未取得":visible.Length+" / "+rows.Length+" 件　　発生回数はSHIRANUIが同じ検知キーでまとめた回数";
   if(alertId!=null)count.Text=visible.Length==0?"通知のアラート ID "+alertId+" は現在の履歴にありません。":"通知のアラート ID "+alertId;
  };
  if(alertId!=null){
   filters.Visibility=Visibility.Collapsed;
   var back=new Button{Content="通常の履歴に戻る",HorizontalAlignment=HorizontalAlignment.Left};
   DockPanel.SetDock(back,Dock.Top);work.Children.Insert(0,back);
   back.Click+=(s,e)=>{alertId=null;filters.Visibility=Visibility.Visible;back.Visibility=Visibility.Collapsed;filter();};
   foreach(var card in cards)card.Click+=(s,e)=>{alertId=null;filters.Visibility=Visibility.Visible;back.Visibility=Visibility.Collapsed;};
  }
    for(int i=0;i<cards.Length;i++){
   int choice=i;
   cards[i].Click+=(s,e)=>{
    reading=true;
    try{
     tabs.SelectedIndex=0;search.Clear();level.SelectedIndex=choice==2?6:0;
     period.SelectedIndex=choice==3?1:0;pending.IsChecked=choice==1;
     process.SelectedIndex=rule.SelectedIndex=host.SelectedIndex=0;
    }finally{reading=false;}
    filter();
   };
  }
  Action load=()=>{
   now=DateTime.UtcNow;object snapshot=null;string source="",errors="";
   liveFeed=false;
   foreach(var path in sourceChoice.SelectedIndex==1?new[]{history}:new[]{feed,history}){
    try{snapshot=ReadGuardSnapshot(path,path==history);source=Path.GetFileName(path);liveFeed=path==feed;break;}
    catch(Exception ex){if(!GuardReadFailure(ex))throw;errors+=Path.GetFileName(path)+" "+(ex is UnauthorizedAccessException?"権限なし":ex is FileNotFoundException || ex is DirectoryNotFoundException?"ファイルなし":"読込失敗")+"　";}
   }
   rows=snapshot==null?null:AlertArray(snapshot,"alerts").Select(r=>new GuardRow{Raw=r}).ToArray();
   stamp.Text=snapshot==null?"履歴未取得　"+errors:"履歴生成　"+AlertTime(snapshot,"generatedUtc")+"　"+source+"　（最新ID順・最大200件）";
   string[] totals=rows==null?null:new[]{rows.Length.ToString(),rows.Any(r=>r.Review==UnknownGuard)?UnknownGuard:rows.Count(r=>r.Review=="未対応").ToString(),
    rows.Any(r=>r.Rank<0)?UnknownGuard:rows.Count(r=>r.Rank>=2).ToString(),rows.Any(r=>r.Last==DateTime.MinValue)?UnknownGuard:rows.Count(r=>r.Last>=now.AddDays(-1) && r.Last<=now).ToString()};
   for(int i=0;i<values.Length;i++)values[i].Text=totals==null || totals[i]==UnknownGuard?UnknownGuard:totals[i]+" 件";
   reading=true;
   try{
    var available=rows??new GuardRow[0];
    SetGuardChoices(process,available.Select(r=>r.Name),"全プロセス");SetGuardChoices(rule,available.Select(r=>r.Rule),"全ルール");
    var hosts=available.Select(r=>GuardValue(r.Raw,"host")).ToArray();SetGuardChoices(host,hosts,"全端末");
    host.Visibility=hosts.Any(v=>v!=UnknownGuard)?Visibility.Visible:Visibility.Collapsed;
   }finally{reading=false;}
   RenderGuardStats(statistics,snapshot,rows,now);filter();
  };
  sourceChoice.SelectionChanged+=(s,e)=>load();
  update.Click+=(s,e)=>load();search.TextChanged+=(s,e)=>filter();level.SelectionChanged+=(s,e)=>filter();period.SelectionChanged+=(s,e)=>filter();
  process.SelectionChanged+=(s,e)=>filter();rule.SelectionChanged+=(s,e)=>filter();host.SelectionChanged+=(s,e)=>filter();
  order.SelectionChanged+=(s,e)=>filter();pending.Checked+=(s,e)=>filter();pending.Unchecked+=(s,e)=>filter();load();return layout;
 }
 static void RenderGuardStats(StackPanel panel,object snapshot,GuardRow[] rows,DateTime now){
  panel.Children.Clear();var learning=Field(snapshot,"learning");
  panel.Children.Add(UsageText("履歴生成時の検知設定　"+(Field(snapshot,"enabled") is bool?((bool)Field(snapshot,"enabled")?"有効":"無効"):UnknownGuard)+"　　通知の最低スコア　"+GuardValue(snapshot,"popupMinimumScore")));
  AlertHeading(panel,"重要度別のアラート件数");
  if(rows==null)panel.Children.Add(UsageText(UnknownGuard));
  else panel.Children.Add(UsageText(String.Join("　　",new[]{"緊急","高","注意","低",UnknownGuard}.Where(level=>level!=UnknownGuard || rows.Any(r=>r.Level==level)).Select(level=>level+" "+rows.Count(r=>r.Level==level)+" 件"))));
  AlertHeading(panel,"端末の観測イベント（バックエンド集計・過去24時間）");
  var observed=Field(learning,"observed") as IList;
  if(observed==null)panel.Children.Add(UsageText(UnknownGuard));
  else if(observed.Count==0)panel.Children.Add(UsageText("0 件"));
  else foreach(var item in observed)panel.Children.Add(UsageText(GuardValue(item,"source")+"　イベント "+GuardValue(item,"id")+"　　"+GuardValue(item,"count")+" 件"));
  AlertHeading(panel,"最終検知の分布（取得履歴・直近24時間）");
  if(rows==null || rows.Any(r=>r.Last==DateTime.MinValue))panel.Children.Add(UsageText(UnknownGuard));
  else for(int i=0;i<6;i++){
   var start=now.AddHours(-24+4*i);var end=start.AddHours(4);int n=rows.Count(r=>r.Last>=start && (i==5?r.Last<=end:r.Last<end));
   panel.Children.Add(UsageText(start.ToLocalTime().ToString("M/d HH:mm")+" ～ "+end.ToLocalTime().ToString("HH:mm")+"　"+n+" 件"));
   panel.Children.Add(new ProgressBar{Value=n,Maximum=Math.Max(1,rows.Length),Height=6,Foreground=Color("#1583A8")});
  }
  AlertHeading(panel,"登録済み判断（SHIRANUI・最新20件）");
  var feedback=Field(learning,"feedback");var items=Field(feedback,"items") as IList;
  if(items==null)panel.Children.Add(UsageText(UnknownGuard));
  else if(items.Count==0)panel.Children.Add(UsageText("0 件"));
  else foreach(var item in items)panel.Children.Add(UsageText(AlertTime(item,"utc")+"　"+GuardValue(item,"rule")+"　"+ReviewState(item)+"　"+GuardValue(item,"signature")));
 }
}
