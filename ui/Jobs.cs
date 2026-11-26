using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.ComponentModel;
using System.Threading;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Documents;
using System.Windows.Navigation;
using System.Windows.Threading;

internal sealed partial class Desktop {
 static readonly string jobsFile=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"RustAIWorkspace","jobs.json");
 static readonly string jobsLog=Path.ChangeExtension(jobsFile,"status.txt");
 Button jobsButton;
 bool jobsReady;
 static bool JobsRunning(){
  System.Threading.Mutex active;
  if(!System.Threading.Mutex.TryOpenExisting(@"Local\KAGARIJobsPatrol",out active))return false;
  active.Dispose();return true;
 }
 static string JobsDay(){return DateTimeOffset.UtcNow.ToOffset(TimeSpan.FromHours(9)).ToString("yyyy-MM-dd",System.Globalization.CultureInfo.InvariantCulture);}
 public sealed class PublicJobBatch {
  public NewsItem[] items {get;set;}
  public string[] details {get;set;}
  public string[] checkedSources {get;set;}
  public string[] errors {get;set;}
 }
 static string SavePublicJobs(PublicJobBatch batch){
  string day=DateTimeOffset.UtcNow.ToOffset(TimeSpan.FromHours(9)).ToString("yyyy-MM-dd");
  var state=UpdateFeed(jobsFile,s=>{
   s.articles.RemoveAll(a=>batch.checkedSources.Contains(a.source));
   foreach(var item in batch.items.Where(i=>s.sources.Contains(i.source))){
    item.url=NewsUrl(item.url);item.collected=day;item.jobCollectedAt=JobsTimestamp();item.collector="public";
    s.articles.RemoveAll(a=>JobKey(a.url)==JobKey(item.url));s.articles.Add(item);
   }
  });
  return "公開ページ "+batch.checkedSources.Length+"件を確認・一覧 "+state.articles.Count+"件"+
   (batch.errors.Length>0?"（一部取得失敗）":"");
 }
 static string JobPrompt(string[] sources,string id){
  return "KAGARI 生成AI案件巡回 "+id+"。指定サイトだけを巡回し、掲載中で生成AIが業務の中心となる求人・業務委託案件を確認してください。Gmailは使用しません。個別の募集詳細を確認し、募集終了・重複・広告を除いてください。jobBodyに募集元で確認した仕事内容の全文（取得できない時は空欄）、summaryに企業名と仕事内容、significanceに報酬・勤務地・リモート可否・契約形態（不明は未記載）、urlに募集詳細URL、sourceに対応する登録URLを記入してください。全サイトを確認したらこのDMのスレッドにJSONで返信。該当なしはitemsを空配列に。アクセス失敗の場合は空配列で代用せず理由を返信してください。分割時はpartとpartsを付けてください。\n"+
   new JavaScriptSerializer().Serialize(new {sources=sources,response_example=new {items=new[]{new {title="案件名",summary="企業・業務内容",jobBody="仕事内容の原文",significance="募集条件",url="https://example.com/jobs/1",source=sources[0]}}}});
 }
 static void JobStatus(string message){File.WriteAllText(jobsLog,DateTimeOffset.Now.ToString("MM/dd HH:mm")+"　"+message);}
 static int RunJobsPatrol(){return RunJobsPatrolWithInterval(TimeSpan.FromMinutes(1));}
 static int RunJobsPatrolWithInterval(TimeSpan replyInterval){
  using(var mutex=new Mutex(false,@"Local\KAGARIJobsPatrol")){
   try{if(!mutex.WaitOne(0))return 0;}catch(AbandonedMutexException){}
   string publicStatus="";
   try{
    var state=LoadFeed(jobsFile);
    if(state.sources.Length==0)return 2;
    PatrolChange(p=>{p.publicPhase="実行中";p.dotsPhase="未依頼";p.targets=state.sources.Length;p.checkedCount=0;p.items=0;p.details=null;});
    JobStatus("公開ページを確認しています。");
    try{
     var batch=NewsScript<PublicJobBatch>("jobs_public.ps1","read",new {sources=state.sources}).GetAwaiter().GetResult();
     publicStatus=SavePublicJobs(batch)+"。";
     PatrolChange(p=>{p.publicPhase=batch.errors.Length==0?"完了":batch.checkedSources.Length>0 || batch.items.Length>0?"部分失敗":"失敗";p.checkedCount=batch.checkedSources.Length;p.items=batch.items.Select(i=>JobKey(i.url)).Distinct().Count();p.details=batch.details??batch.errors;});
    }catch(Exception ex){PatrolChange(p=>{p.publicPhase="失敗";p.details=new[]{ex.Message};});publicStatus="公開ページ取得失敗　"+ex.Message+"。";}
    var receipt=state.pendingDots;
    long sent;
    if(receipt!=null && long.TryParse((receipt.ts??"").Split('.')[0],out sent)){
     var sentDay=new DateTime(1970,1,1,0,0,0,DateTimeKind.Utc).AddSeconds(sent).AddHours(9).Date;
     if(sentDay<DateTime.UtcNow.AddHours(9).Date)receipt=null;
    }
    if(receipt==null){
     string id=Guid.NewGuid().ToString("N");
     JobStatus(publicStatus+"Dotsに巡回を依頼しています。");
     receipt=SlackNewsCall("send",new {collector="dots",prompt=JobPrompt(state.sources,id)}).GetAwaiter().GetResult();
     receipt.id=id;UpdateFeed(jobsFile,s=>{if(s.sources.SequenceEqual(state.sources))s.pendingDots=receipt;});
    }
    PatrolChange(p=>p.dotsPhase="返信待ち");
    for(int pass=0;pass<11;pass++){
     JobStatus(publicStatus+"Dots返信確認 "+(pass+1)+"/11");
     var current=LoadFeed(jobsFile).pendingDots;
     if(current==null || current.id!=receipt.id){PatrolChange(p=>p.dotsPhase="中断");JobStatus("登録設定が変更されたため巡回を終了しました。");return 0;}
     var response=SlackNewsCall("read",new {collector="dots",receipt.channel,receipt.ts,receipt.id}).GetAwaiter().GetResult();
     string partial;
     var items=CollectReply(response.replies??new string[0],"dots",out partial);
     if(items!=null){
      string date=DateTimeOffset.UtcNow.ToOffset(TimeSpan.FromHours(9)).ToString("yyyy-MM-dd");
      if(items.Any(i=>!state.sources.Contains(i.source)))throw new FormatException("返信に登録元を特定できない案件があります。");
      var jobs=items.GroupBy(i=>JobKey(i.url)).Select(g=>g.First()).ToList();
      foreach(var i in jobs){i.collected=date;i.jobCollectedAt=JobsTimestamp();}
      var saved=UpdateFeed(jobsFile,s=>{if(s.pendingDots!=null && s.pendingDots.id==receipt.id){
       var direct=s.articles.Where(a=>a.collector=="public" && a.collected==date).ToDictionary(a=>JobKey(a.url));
       foreach(var entry in jobs){
        NewsItem article;
        if(direct.TryGetValue(JobKey(entry.url),out article) && String.IsNullOrWhiteSpace(entry.jobBody))entry.jobBody=article.jobBody;
       }
       jobs.AddRange(s.articles.Where(a=>a.collector=="public" && a.collected==date && !jobs.Any(j=>JobKey(j.url)==JobKey(a.url))).ToList());
       s.articles=jobs;s.pendingDots=null;
      }});
      PatrolChange(p=>{
       p.dotsPhase="受信（"+items.Select(i=>JobKey(i.url)).Distinct().Count()+"件）";
       p.details=(p.details??new string[0]).Concat(items.GroupBy(i=>i.source).Select(g=>g.Key+"　Dots返信 "+g.Select(i=>JobKey(i.url)).Distinct().Count()+"件")).ToArray();
      });
      JobStatus("巡回完了　"+saved.articles.Count+"件");return 0;
     }
     JobStatus(publicStatus+(partial??(response.replies!=null && response.replies.Length>0?"Dots返信あり・案件一覧待ち":"Dots返信なし"))+"（確認 "+(pass+1)+"/11）");
     if(pass<10)Thread.Sleep(replyInterval);
    }
    PatrolChange(p=>p.dotsPhase="タイムアウト");
    JobStatus(publicStatus+"Dotsの確認時間が終了しました。今日中は「今すぐ巡回」で同じ返信を再確認できます。");
    return 1;
   }catch(Exception ex){
    PatrolChange(p=>p.dotsPhase="失敗");
    UpdateFeed(jobsFile,s=>s.pendingDots=null);
    JobStatus(publicStatus+"Dots巡回未完了　"+ex.Message);return 1;
   }finally{mutex.ReleaseMutex();}
  }
 }
 void ShowJobs(){
  ShowWorkspace();
  At<Grid>("Workspace").Visibility=Visibility.Collapsed;
  foreach(var button in At<StackPanel>("Rail").Children.OfType<Button>())Selector.SetIsSelected(button,button==jobsButton);
  At<Border>("Jobs").Visibility=Visibility.Visible;
  if(!jobsReady){jobsReady=true;InitJobs();}
 }
 async void InitJobs(){
  var sources=At<TextBox>("JobsSources");var time=At<TextBox>("JobsTime");
  var daily=At<CheckBox>("JobsDaily");var status=At<TextBlock>("JobsStatus");
  var settings=At<Expander>("JobsSettings");var list=At<DataGrid>("JobsList");
  var search=At<TextBox>("JobsSearch");var count=At<TextBlock>("JobsCount");
  var site=At<ComboBox>("JobsSite");var pay=At<ComboBox>("JobsPay");var work=At<ComboBox>("JobsWork");var order=At<ComboBox>("JobsOrder");
  var buttons=new[]{"All","New","Pinned","Unread"}.Select(n=>At<Button>("Jobs"+n)).ToArray();
  var labels=new[]{"全案件","新着","ピン留め","未確認"};
  var counts=new TextBlock[buttons.Length];
  for(int i=0;i<buttons.Length;i++){
   var tile=new StackPanel{Orientation=Orientation.Horizontal};
   tile.Children.Add(new TextBlock{Text=labels[i],VerticalAlignment=VerticalAlignment.Center,Margin=new Thickness(0,0,14,0)});
   counts[i]=new TextBlock{FontSize=21,FontWeight=FontWeights.SemiBold};tile.Children.Add(counts[i]);
   buttons[i].Content=tile;System.Windows.Automation.AutomationProperties.SetName(buttons[i],labels[i]);
  }
  var panes=At<Grid>("JobsPanes");var grip=At<GridSplitter>("JobsGrip");var detail=At<Border>("JobsDetail");
  var body=At<ScrollViewer>("JobsDetailScroll");
  var save=At<Button>("JobsSave");var run=At<Button>("JobsNow");
  var rows=new List<NewsItem>();int mode=0;bool drawing=false,hasSources=false,closed=false;
  System.Diagnostics.Process launch=null;
  GridLength left=new GridLength(68,GridUnitType.Star),right=new GridLength(32,GridUnitType.Star);
  Action layout=()=>{
   bool narrow=panes.ActualWidth<1100;
   bool show=narrow?detail.DataContext!=null:!closed;
   if(grip.Visibility==Visibility.Visible && panes.ColumnDefinitions[2].Width.IsStar){
    left=panes.ColumnDefinitions[0].Width;right=panes.ColumnDefinitions[2].Width;
   }
   panes.ColumnDefinitions[0].Width=show && !narrow?left:new GridLength(1,GridUnitType.Star);
   panes.ColumnDefinitions[1].Width=new GridLength(show && !narrow?8:0);
   panes.ColumnDefinitions[2].Width=show && !narrow?right:new GridLength(0);
   Grid.SetColumn(detail,narrow?0:2);Grid.SetColumnSpan(detail,narrow?3:1);
   detail.Visibility=Visible(show);grip.Visibility=Visible(show && !narrow);
   body.Visibility=Visible(detail.DataContext!=null);At<TextBlock>("JobsDetailHint").Visibility=Visible(detail.DataContext==null);
  };
  panes.SizeChanged+=delegate{layout();};
  Action clearDetail=()=>{list.SelectedItem=null;detail.DataContext=null;closed=true;layout();};
  At<Button>("JobsClose").Click+=delegate{clearDetail();};
  At<Button>("JobsLink").Click+=delegate{
   var job=detail.DataContext as NewsItem;if(job==null)return;
   try{System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(NewsUrl(job.url)){UseShellExecute=true});}
   catch(Exception ex){status.Text=ex.Message;}
  };
  site.Items.Add("掲載サイト");site.SelectedIndex=0;
  pay.ItemsSource=new[]{"報酬形態","固定","月額","時給","見積り","未取得"};pay.SelectedIndex=0;
  work.Items.Add("稼働条件");work.SelectedIndex=0;
  order.ItemsSource=new[]{"収集日順","案件名順","掲載サイト順"};order.SelectedIndex=0;
  Action filter=()=>{
   if(drawing)return;
   var words=search.Text.Split((char[])null,StringSplitOptions.RemoveEmptyEntries);
   list.Items.Filter=value=>{
    var job=(NewsItem)value;
    if(mode==1 && !job.IsNewJob || mode==2 && !job.IsJobPinned || mode==3 && job.JobReviewed)return false;
    if(site.SelectedIndex>0 && job.JobSite!=(string)site.SelectedItem || pay.SelectedIndex>0 && job.JobPayType!=(string)pay.SelectedItem)return false;
    if(work.SelectedIndex>0 && (string)work.SelectedItem!=job.JobWork && (string)work.SelectedItem!=job.JobDays)return false;
    string text=String.Join(" ",job.title,job.DisplayJobBody,job.significance,job.JobSite,job.source,job.collected);
    return words.All(word=>text.IndexOf(word,StringComparison.OrdinalIgnoreCase)>=0);
   };
   count.Text=list.Items.Count+" 件";
   int[] totals={rows.Count,rows.Count(j=>j.IsNewJob),rows.Count(j=>j.IsJobPinned),rows.Count(j=>!j.JobReviewed)};
   for(int n=0;n<buttons.Length;n++){
    counts[n].Text=totals[n].ToString();
    buttons[n].Background=Color(n==mode?"#096C65":"#FFFFFF");buttons[n].Foreground=Color(n==mode?"#FFFFFF":"#202B35");
   }
   var empty=At<TextBlock>("JobsEmpty");empty.Text=rows.Count==0?"収集した案件はまだありません。":"条件に一致する案件がありません。";
   empty.Visibility=Visible(list.Items.Count==0);
  };
  Action sort=()=>{
   using(list.Items.DeferRefresh()){
    list.Items.SortDescriptions.Clear();
    list.Items.SortDescriptions.Add(new SortDescription("IsJobPinned",ListSortDirection.Descending));
    string key=order.SelectedIndex==1?"title":order.SelectedIndex==2?"JobSite":"JobTime";
    list.Items.SortDescriptions.Add(new SortDescription(key,order.SelectedIndex==0?ListSortDirection.Descending:ListSortDirection.Ascending));
   }
  };
  for(int n=0;n<buttons.Length;n++){int index=n;buttons[n].Click+=delegate{mode=index;clearDetail();filter();};}
  Action changeFilter=()=>{
   if(drawing)return;
   filter();var shown=detail.DataContext as NewsItem;
   if(shown!=null && !list.Items.Contains(shown))clearDetail();
  };
  search.TextChanged+=delegate{changeFilter();};site.SelectionChanged+=delegate{changeFilter();};pay.SelectionChanged+=delegate{changeFilter();};
  work.SelectionChanged+=delegate{changeFilter();};order.SelectionChanged+=delegate{sort();};
  At<Button>("JobsReset").Click+=delegate{drawing=true;mode=0;search.Clear();site.SelectedIndex=pay.SelectedIndex=work.SelectedIndex=order.SelectedIndex=0;drawing=false;clearDetail();sort();filter();};
  Action<ComboBox,string,IEnumerable<string>> options=(box,title,values)=>{
   string selected=box.SelectedItem as string;
   box.ItemsSource=null;box.Items.Clear();box.ItemsSource=new[]{title}.Concat(values.Where(v=>!String.IsNullOrEmpty(v)).Distinct().OrderBy(v=>v)).ToList();
   box.SelectedItem=selected;if(box.SelectedIndex<0)box.SelectedIndex=0;
  };
  DateTime lastData=DateTime.MinValue,lastLog=DateTime.MinValue;string displayedDay=null;
  JobPatrolInfo patrol=null;
  Action refresh=()=>{
   var version=File.GetLastWriteTimeUtc(jobsFile);string today=JobsDay();
   if(File.Exists(jobsFile) && (version!=lastData || today!=displayedDay)){
    var state=LoadFeed(jobsFile);hasSources=state.sources.Length>0;patrol=state.jobPatrol;
    var currentJob=detail.DataContext as NewsItem;
    rows=state.articles.ToList();
    if(state.pinnedJobs!=null){
     var current=new HashSet<string>(rows.Select(i=>JobKey(i.url)));
     rows.AddRange(state.pinnedJobs.Values.Where(i=>!current.Contains(JobKey(i.url))));
    }
    foreach(var job in rows){
     string firstSeen,key=JobKey(job.url);
     job.IsJobPinned=state.pinnedJobs!=null && state.pinnedJobs.ContainsKey(key);
     job.JobReviewed=state.reviewedJobs!=null && state.reviewedJobs.Contains(key);
     job.IsNewJob=state.jobFirstSeen!=null && state.jobFirstSeen.TryGetValue(key,out firstSeen) && firstSeen==today;
    }
    drawing=true;
    try{
     options(site,"掲載サイト",rows.Select(j=>j.JobSite));
     options(work,"稼働条件",rows.SelectMany(j=>new[]{j.JobWork,j.JobDays}).Concat(new[]{"未取得"}));
     list.ItemsSource=rows;sort();
    }finally{drawing=false;}
    filter();
    if(currentJob!=null){
     var selected=rows.FirstOrDefault(j=>JobKey(j.url)==JobKey(currentJob.url));
     drawing=true;try{list.SelectedItem=selected!=null && list.Items.Contains(selected)?selected:null;detail.DataContext=selected;}finally{drawing=false;}
    }
    lastData=version;displayedDay=today;
   }
   version=File.GetLastWriteTimeUtc(jobsLog);
   if(File.Exists(jobsLog) && version!=lastLog){status.Text=File.ReadAllText(jobsLog);lastLog=version;}
   if(launch!=null && launch.HasExited){launch.Dispose();launch=null;}
   bool busy=launch!=null || JobsRunning();
   run.IsEnabled=hasSources && !busy;
   string phase=patrol==null?"記録なし":patrol.publicPhase;
   string dots=patrol==null?"記録なし":patrol.dotsPhase;
   if(patrol==null){
    var prior=System.Text.RegularExpressions.Regex.Match(status.Text,@"公開ページ\s*(\d+)件を確認");
    if(prior.Success)phase="確認 "+prior.Groups[1].Value+" URL（前回記録）";
    if(status.Text.Contains("確認時間が終了") || status.Text.Contains("確認時間内に届"))dots="タイムアウト";
    if(busy)phase="旧版で巡回中";
   }
   if(!busy && phase=="実行中")phase="中断";
   if(!busy && dots=="返信待ち")dots="中断・再確認可";
   At<TextBlock>("JobsPublic").Text="公開巡回　"+phase+(patrol==null?"":"　登録 "+patrol.targets+" URL ／ 確認 "+patrol.checkedCount+" ／ 取得 "+patrol.items+"件");
   At<TextBlock>("JobsDots").Text="Dots　"+dots;
   At<TextBlock>("JobsSitesReport").Text=patrol==null?"サイト別の記録は次の巡回から表示します。":"更新 "+patrol.updated+"（日本時間）\n"+String.Join("\n",patrol.details??new string[0]);
   if(!busy && patrol!=null && (patrol.publicPhase=="実行中" || patrol.dotsPhase=="返信待ち"))status.Text="巡回は中断されました。「今すぐ巡回」で再確認できます。";
   layout();
  };
  list.SelectionChanged+=(s,e)=>{
   if(drawing)return;
   var job=list.SelectedItem as NewsItem;
   if(job==null)return;
   detail.DataContext=job;closed=false;body.ScrollToHome();layout();
   if(job.JobReviewed)return;
   try{
    UpdateFeed(jobsFile,state=>{if(state.reviewedJobs==null)state.reviewedJobs=new List<string>();if(!state.reviewedJobs.Contains(JobKey(job.url)))state.reviewedJobs.Add(JobKey(job.url));});
    job.JobReviewed=true;job.NotifyJobByline();filter();
   }catch(Exception ex){status.Text=ex.Message;}
  };
  filter();
  try{
   var initial=LoadFeed(jobsFile);
   sources.Text=String.Join(Environment.NewLine,initial.sources);settings.IsExpanded=initial.sources.Length==0;
   hasSources=initial.sources.Length>0;run.IsEnabled=hasSources;
   status.Text=run.IsEnabled?"巡回結果がここに表示されます。":"巡回先の求人サイトを登録してください。";
   refresh();
   list.PreviewMouseLeftButtonDown+=(sender,e)=>{
    var node=e.OriginalSource as System.Windows.DependencyObject;
    while(node is System.Windows.Media.Visual && node!=list){
     var pin=node as ToggleButton;
     if(pin!=null && Equals(pin.Tag,"JobPin")){
      e.Handled=true;pin.SetCurrentValue(ToggleButton.IsCheckedProperty,pin.IsChecked!=true);pin.RaiseEvent(new RoutedEventArgs(ButtonBase.ClickEvent,pin));return;
     }
     node=System.Windows.Media.VisualTreeHelper.GetParent(node);
    }
   };
   At<Border>("Jobs").AddHandler(ButtonBase.ClickEvent,new RoutedEventHandler(delegate(object sender,RoutedEventArgs e){
    var button=e.OriginalSource as ToggleButton;
    if(button==null || !Equals(button.Tag,"JobPin"))return;
    e.Handled=true;
    var job=(NewsItem)button.DataContext;
    try{
     UpdateFeed(jobsFile,s=>{
      if(s.pinnedJobs==null)s.pinnedJobs=new Dictionary<string,NewsItem>();
      if(button.IsChecked==true)s.pinnedJobs[JobKey(job.url)]=job;else s.pinnedJobs.Remove(JobKey(job.url));
     });
     lastData=DateTime.MinValue;refresh();
    }catch(Exception ex){button.SetCurrentValue(ToggleButton.IsCheckedProperty,job.IsJobPinned);status.Text=ex.Message;}
   }));
   var timer=new DispatcherTimer{Interval=TimeSpan.FromSeconds(3)};
   timer.Tick+=delegate{if(panes.IsVisible)try{refresh();}catch(IOException){}};
   timer.Start();window.Closed+=delegate{timer.Stop();};
   list.AddHandler(Hyperlink.RequestNavigateEvent,new RequestNavigateEventHandler(delegate(object sender,RequestNavigateEventArgs e){
    e.Handled=true;try{System.Diagnostics.Process.Start(NewsUrl(e.Uri.AbsoluteUri));}catch(Exception ex){status.Text=ex.Message;}
   }));
   run.Click+=delegate{
    try{if(!run.IsEnabled)return;run.IsEnabled=false;launch=System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(Path.Combine(folder,"KAGARI.exe"),"--jobs-patrol"){UseShellExecute=false,CreateNoWindow=true});
     lastLog=DateTime.MinValue;refresh();}
    catch(Exception ex){run.IsEnabled=hasSources && !JobsRunning();status.Text=ex.Message;}
   };
   save.Click+=async delegate{
    save.IsEnabled=false;
    try{
     var urls=sources.Text.Split(new[]{'\r','\n'},StringSplitOptions.RemoveEmptyEntries).Where(v=>v.Trim().Length>0).Select(NewsUrl).Distinct().ToArray();
     if(urls.Length==0 && daily.IsChecked==true)throw new FormatException("巡回先URLを入力してください。");
     var schedule=await NewsScript<PatrolSettings>("news_schedule.ps1","set jobs",new {time=time.Text.Trim(),agents="dots",enabled=daily.IsChecked==true});
     UpdateFeed(jobsFile,s=>{if(!s.sources.SequenceEqual(urls)){s.pendingDots=null;s.articles.RemoveAll(i=>!urls.Contains(i.source));}s.sources=urls;});
     hasSources=urls.Length>0;refresh();
     status.Text=schedule.enabled?"保存しました。次回巡回 "+schedule.next:"保存しました。定期巡回は停止中です。";
    }catch(Exception ex){status.Text=ex.Message;}finally{save.IsEnabled=true;}
   };
   save.IsEnabled=false;
   var config=await NewsScript<PatrolSettings>("news_schedule.ps1","get jobs",new {});
   daily.IsChecked=config.enabled || (initial.sources.Length==0 && !File.Exists(jobsFile));time.Text=config.time;
  }catch(Exception ex){status.Text=ex.Message;}finally{save.IsEnabled=true;}
 }
}
