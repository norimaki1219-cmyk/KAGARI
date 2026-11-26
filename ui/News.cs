using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Text;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Controls;

internal sealed partial class Desktop {
 static readonly string[] newsCategories={"モデル・研究","製品・機能","開発ツール","安全性・ガバナンス","産業・活用","ニュースダイジェスト","その他"};
 public sealed partial class NewsItem : System.ComponentModel.INotifyPropertyChanged {
  bool unread;
  [ScriptIgnore] public bool IsUnread {get{return unread;}}
  public event System.ComponentModel.PropertyChangedEventHandler PropertyChanged;
  internal void SetUnread(bool value){
   if(unread==value)return;
   unread=value;
   var changed=PropertyChanged;
   if(changed!=null)changed(this,new System.ComponentModel.PropertyChangedEventArgs("IsUnread"));
  }
  public bool sharedSource {get;set;}
  [ScriptIgnore] public string ReadKey {get{return sharedSource?url+"\n"+title:url;}}
  public string event_url {get;set;}
  [ScriptIgnore] public List<NewsItem> Sources {get;set;}
  public NewsItem AsStory(List<NewsItem> sources){var story=(NewsItem)MemberwiseClone();story.Sources=sources;return story;}
  public string collected {get;set;}
  public string category {get;set;}
  public string title {get;set;}
  public string summary {get;set;}
  public string significance {get;set;}
  public string url {get;set;}
  public string collector {get;set;}
  public string source {get;set;}
  [ScriptIgnore] public bool IsJobPinned {get;set;}
  [ScriptIgnore] public bool IsNewJob {get;set;}
  [ScriptIgnore] public string JobSite {get{
   Uri site;if(!Uri.TryCreate(source,UriKind.Absolute,out site) && !Uri.TryCreate(url,UriKind.Absolute,out site))return "";
   if(site.Host=="coconala.com")return "ココナラ";
   if(site.Host=="freelance.findy-code.io")return "Findy Freelance";
   return site.Host;
  }}
  public string Byline {get{
   if(Sources!=null && Sources.Count>1)return String.Join("・",Sources.Select(i=>i.collector=="muse"?"Muse":"Dots").Distinct())+"  ·  "+Sources.Count+"件の収集結果";
   Uri link;string name=collector=="muse"?"Muse":"Dots";
   if(!Uri.TryCreate(String.IsNullOrEmpty(source)?url:source,UriKind.Absolute,out link))return name;
   string path=Uri.UnescapeDataString(link.AbsolutePath).Trim('/');
   return name+"  ·  "+(path.StartsWith("@")?path.Split('/')[0]:link.Host);
  }}
  public override string ToString(){return title;}
 }
 public sealed class NewsBatch { public int part {get;set;} public int parts {get;set;} public NewsItem[] items {get;set;} }
 public sealed class NewsReceipt {
  public string id {get;set;}
  public string channel {get;set;}
  public string ts {get;set;}
  public string[] replies {get;set;}
 }
 public sealed class NewsState {
  public List<string> reviewedJobs;
  public JobPatrolInfo jobPatrol;
  public Dictionary<string,NewsItem> pinnedJobs;
  public Dictionary<string,string> jobFirstSeen;
  public Dictionary<string,string> lastPatrol=new Dictionary<string,string>();
  public Dictionary<string,NewsReceipt> patrolReceipts=new Dictionary<string,NewsReceipt>();
  public List<string> readNewsUrls=new List<string>();
  public NewsReceipt pendingDots;
  public NewsReceipt pendingMuse;
  public NewsReceipt Pending(string who){return who=="muse"?pendingMuse:pendingDots;}
  public void SetPending(string who,NewsReceipt value){if(who=="muse")pendingMuse=value;else pendingDots=value;}
  public string[] sources = { "https://www.youtube.com/@AI仙人ch", "https://www.youtube.com/@AIAIChatGPT-cj4sh", "https://www.youtube.com/@marketAI-pf7ih", "https://www.youtube.com/@ai_tech_broadcast" };
  public Dictionary<string,string> endpoints = new Dictionary<string,string>();
  public List<NewsItem> articles = new List<NewsItem>();
 }
 static async System.Threading.Tasks.Task<NewsReceipt> SlackNewsCall(string action,object payload){
  return await NewsScript<NewsReceipt>("news_dots.ps1",action,payload);
 }
 static async System.Threading.Tasks.Task<T> NewsScript<T>(string file,string action,object payload){
  var script=Path.Combine(folder,"src",file);
  var exe=Path.Combine(Environment.SystemDirectory,@"WindowsPowerShell\v1.0\powershell.exe");
  var start=new System.Diagnostics.ProcessStartInfo(exe,"-NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File \""+script+"\" "+action){
   UseShellExecute=false,CreateNoWindow=true,RedirectStandardInput=true,RedirectStandardOutput=true,RedirectStandardError=true,
   StandardOutputEncoding=Encoding.UTF8,StandardErrorEncoding=Encoding.UTF8
  };
  using(var process=System.Diagnostics.Process.Start(start)){
   var output=process.StandardOutput.ReadToEndAsync();var error=process.StandardError.ReadToEndAsync();
   var bytes=Encoding.UTF8.GetBytes(new JavaScriptSerializer().Serialize(payload));
   await process.StandardInput.BaseStream.WriteAsync(bytes,0,bytes.Length);process.StandardInput.Close();
   await System.Threading.Tasks.Task.WhenAll(output,error);process.WaitForExit();
   if(process.ExitCode!=0)throw new IOException(error.Result);
   return new JavaScriptSerializer{MaxJsonLength=file=="jobs_public.ps1"?int.MaxValue:2097152}.Deserialize<T>(output.Result);
  }
 }
 static string SlackNewsJson(string message){
  string text=System.Net.WebUtility.HtmlDecode(message);
  text=System.Text.RegularExpressions.Regex.Replace(text,@"<(https?://[^|>\s]+)(?:\|([^>]*))?>",m=>{
   var url=m.Groups[1].Value;var label=m.Groups[2].Value;
   // Slack can autolink across adjacent JSON fields and preserve the original delimiters in the label.
   return url.IndexOf("%22",StringComparison.OrdinalIgnoreCase)>=0 && label.Contains("\",\"")?label:url;
  });
  int start=text.IndexOf('{'),end=text.LastIndexOf('}');
  return start<0 || end<start?"":text.Substring(start,end-start+1);
 }
 static string NewsUrl(string raw){
  Uri u;
  if(!Uri.TryCreate((raw??"").Trim(),UriKind.Absolute,out u) || (u.Scheme!="https" && u.Scheme!="http") || u.UserInfo.Length>0 || String.IsNullOrEmpty(u.Host))
   throw new FormatException("http または https のURLを指定してください。");
  return u.AbsoluteUri;
 }
 static string ArticleKey(string raw){
  Uri u;if(!Uri.TryCreate(raw,UriKind.Absolute,out u) || (u.Scheme!="http" && u.Scheme!="https") || u.AbsolutePath=="/")return null;
  string path=u.AbsolutePath.TrimEnd('/');
  if(u.Host=="openai.com")path=System.Text.RegularExpressions.Regex.Replace(path,@"^/[a-z]{2}-[a-z]{2}/","/",System.Text.RegularExpressions.RegexOptions.IgnoreCase);
  if(u.Host=="youtu.be")return "youtube:"+path.TrimStart('/');
  if(u.Host=="youtube.com" || u.Host=="www.youtube.com"){
   if(path.StartsWith("/embed/") || path.StartsWith("/shorts/"))return "youtube:"+path.Split('/').Last();
   var id=System.Text.RegularExpressions.Regex.Match(u.Query,@"(?:[?&])v=([A-Za-z0-9_-]+)");if(id.Success)return "youtube:"+id.Groups[1].Value;
  }
  return u.Host+path+u.Query;
 }
 static List<NewsItem> NewsStories(List<NewsItem> articles){
  var byKey=new Dictionary<string,List<NewsItem>>();
  var groups=new List<List<NewsItem>>();
  foreach(var item in articles){
   var keys=new[]{item.sharedSource?null:ArticleKey(item.url),ArticleKey(item.event_url),"title:"+item.title.Trim()}.Where(k=>k!=null).Distinct().ToList();
   var matches=keys.Where(byKey.ContainsKey).Select(k=>byKey[k]).Distinct().ToList();
   var group=matches.FirstOrDefault();
   if(group==null){group=new List<NewsItem>();groups.Add(group);}
   foreach(var merged in matches.Skip(1)){
    group.AddRange(merged);groups.Remove(merged);
    foreach(var key in byKey.Where(pair=>pair.Value==merged).Select(pair=>pair.Key).ToList())byKey[key]=group;
   }
   group.Add(item);foreach(var key in keys)byKey[key]=group;
  }
  return groups.Select(g=>g[0].AsStory(g)).ToList();
 }
 static string NewsRequest(string agent,string[] sources){
  return new JavaScriptSerializer().Serialize(new {
   collector=agent, sources=sources, instruction="最近のAIニュースと、登録URLの最新の内容を調査してください。一次資料を優先し、技術・運用上の判断に影響する新情報だけを選び、重複を除き、件数上限なしで条件に合う情報をすべて返してください。categoryに主題を1つ指定（"+String.Join("／",newsCategories)+"）。Slackで長い場合はスレッド内で分割し、各JSONにpart（1始まり）とparts（総数）を指定。同じ出来事については統合し、event_urlにはその出来事に固有の公式発表URLを記入（特定できない時・総集編は空文字。企業トップや製品ホームは不可）。各項目に日本語の見出し、具体的な要約、重要な理由、原文URL、参照した登録URL（ニュース検索から見つけた場合は空文字）を記入。YouTubeでは動画本編・字幕または投稿者の説明文を確認し、説明文のみの場合はその旨を明記し、個別動画のURLを原文URLに記入。タイトルだけから内容を推測しない。内容を確認できない項目は除き、該当なしなら空配列。以下の形式のJSONのみを返してください。",
   response_example=new {items=new[]{new {event_url="",category="製品・機能",title="見出し",summary="要約",significance="重要な理由",url="https://example.com/article",source=""}}}
  });
 }
 static List<NewsItem> CollectReply(string[] replies,string who,out string partial){
    var chunks=new SortedDictionary<int,NewsBatch>();
    int expected=0;
    foreach(var message in replies.Reverse()){
     var json=SlackNewsJson(message);if(json.Length==0)continue;
     NewsBatch batch;
     try{batch=ParseNews(json,who);}catch(FormatException){continue;}catch(ArgumentException){continue;}catch(InvalidOperationException){continue;}
     if(batch.parts==0 && batch.part==0){batch.part=1;batch.parts=1;}
     if(batch.part<1 || batch.part>batch.parts || (expected!=0 && expected!=batch.parts))continue;
     expected=batch.parts;
     if(!chunks.ContainsKey(batch.part))chunks.Add(batch.part,batch);
     if(chunks.Count==expected)break;
    }
  partial=expected>0?"分割返信 "+chunks.Count+"/"+expected+" 件を受信":null;
  if(expected==0 || chunks.Count!=expected)return null;
  var combined=chunks.Values.SelectMany(chunk=>chunk.items).ToList();
  MarkSharedSources(combined);return combined;
 }
 public sealed class PatrolSettings{
  public bool enabled {get;set;}
  public string time {get;set;}
  public string agents {get;set;}
  public string next {get;set;}
 }
 static string PatrolWho(string key){return key.Substring(key.LastIndexOf('/')+1);}
 static async System.Threading.Tasks.Task<bool> ReceivePatrol(string key,NewsReceipt receipt){
  string who=PatrolWho(key),partial;
  var response=await SlackNewsCall("read",new {receipt.id,receipt.channel,receipt.ts,collector=who});
  var items=CollectReply(response.replies??new string[0],who,out partial);
  if(items==null)return false;
  UpdateNews(s=>{
   if(!s.patrolReceipts.Remove(key))return;
   foreach(var item in items.AsEnumerable().Reverse()){
    s.articles.RemoveAll(old=>old.ReadKey==item.ReadKey && old.collector==who);s.articles.Insert(0,item);
   }
  });
  return true;
 }
 static async System.Threading.Tasks.Task SendPatrol(string who,string date){
  string key=date+"/"+who;bool reserved=false;
  var state=UpdateNews(s=>{
   string last;if(s.lastPatrol.TryGetValue(who,out last) && last==date)return;
   s.lastPatrol[who]=date;reserved=true;
  });
  if(!reserved)return;
  string id=Guid.NewGuid().ToString("N");
  string prompt="KAGARI AIニュース定期巡回 "+key+" "+id+"。最近の更新を確認し、既報と同じ内容は除いてください。結果JSONはこのDMのスレッドへ。\n"+NewsRequest(who,state.sources);
  var receipt=await SlackNewsCall("send",new {prompt=prompt,collector=who});
  receipt.id=id;UpdateNews(s=>s.patrolReceipts[key]=receipt);
 }
 static int ReceiveScheduledDots(string day){
  DateTime date;
  if(!DateTime.TryParseExact(day,"yyyy-MM-dd",System.Globalization.CultureInfo.InvariantCulture,System.Globalization.DateTimeStyles.None,out date))return 2;
  try{
   string key=day+"/dots";
   for(int pass=0;pass<11;pass++){
    NewsReceipt receipt;
    if(!LoadNews().patrolReceipts.TryGetValue(key,out receipt))return 0;
    if(ReceivePatrol(key,receipt).GetAwaiter().GetResult())return 0;
    if(pass<10)System.Threading.Thread.Sleep(TimeSpan.FromSeconds(65));
   }
   return 0; // The saved receipt remains available to the UI and the following patrol.
  }catch(Exception ex){
   File.WriteAllText(Path.Combine(Path.GetDirectoryName(newsFile),"news-dots-receive-last.txt"),DateTimeOffset.Now.ToString("u")+" "+ex.Message);return 1;
  }
 }
 static int RunNewsPatrol(string agents){
  if(agents!="both" && agents!="muse" && agents!="dots")return 2;
  string day=DateTimeOffset.UtcNow.ToOffset(TimeSpan.FromHours(9)).ToString("yyyy-MM-dd");
  var errors=new List<string>();
  foreach(var who in agents=="both"?new[]{"muse","dots"}:new[]{agents}){
   try{SendPatrol(who,day).GetAwaiter().GetResult();}catch(Exception ex){errors.Add(who+" "+ex.Message);}
  }
  try{
   var waiting=LoadNews().patrolReceipts.ToArray();
   // Older replies are checked once per patrol. Today's requests get a short background polling window.
   for(int pass=0;pass<11 && waiting.Length>0;pass++){
    var retry=new List<KeyValuePair<string,NewsReceipt>>();
    foreach(var entry in waiting){
     try{
      if(!ReceivePatrol(entry.Key,entry.Value).GetAwaiter().GetResult() && entry.Key.StartsWith(day+"/"))retry.Add(entry);
     }catch(Exception ex){errors.Add(entry.Key+" "+ex.Message);}
    }
    waiting=retry.ToArray();
    if(waiting.Length>0 && pass<10)System.Threading.Thread.Sleep(TimeSpan.FromSeconds(65));
   }
  }catch(Exception ex){errors.Add(ex.Message);}
  File.WriteAllText(Path.Combine(Path.GetDirectoryName(newsFile),"news-patrol-last.txt"),
   DateTimeOffset.Now.ToString("u")+" "+(errors.Count==0?"巡回処理完了":String.Join(Environment.NewLine,errors)));
  return errors.Count==0?0:1;
 }
 async void InitPatrol(TextBlock status){
  var controls=At<WrapPanel>("PatrolControls");
  var enabled=At<CheckBox>("PatrolEnabled");var time=At<TextBox>("PatrolTime");
  
  Action<PatrolSettings> show=c=>{
   enabled.IsChecked=c.enabled;time.Text=c.time;
   At<TextBlock>("PatrolNext").Text=c.enabled?"次回 "+c.next:"停止中";
  };
  var save=At<Button>("PatrolSave");
  save.Click+=async delegate{
   controls.IsEnabled=false;
   try{show(await NewsScript<PatrolSettings>("news_schedule.ps1","set",new {enabled=enabled.IsChecked==true,time=time.Text.Trim(),agents="muse"}));status.Text="巡回設定を保存しました。";}
   catch(Exception ex){status.Text=ex.Message;}finally{controls.IsEnabled=true;}
  };
  controls.IsEnabled=false;
  try{show(await NewsScript<PatrolSettings>("news_schedule.ps1","get",new {}));controls.IsEnabled=true;}
  catch(Exception ex){status.Text="Windowsの巡回設定を取得できません。"+ex.Message;}
 }
 static string NewsField(string value,int max){
  if(String.IsNullOrWhiteSpace(value)||value.Length>max)throw new FormatException("収集結果の必須項目が空か、長すぎます。");
  return value.Trim();
 }
 static List<NewsItem> ReadNews(string json,string agent){
  return ParseNews(json,agent).items.ToList();
 }
 static void MarkSharedSources(IEnumerable<NewsItem> items){
  foreach(var group in items.GroupBy(i=>i.url)){
   if(group.Select(i=>i.title).Distinct().Skip(1).Any())foreach(var item in group)item.sharedSource=true;
  }
 }
 static NewsBatch ParseNews(string json,string agent){
  if(json.Length>1000000)throw new FormatException("結果が大きすぎます。");
  var batch=new JavaScriptSerializer().Deserialize<NewsBatch>(json);
  if(batch==null || batch.items==null)throw new FormatException("記事一覧を含む担当からの返信を選んでください。Dotsへの新規依頼は「Dotsで収集」を押してください。");
  foreach(var item in batch.items){
   if(item==null)throw new FormatException("記事が空です。");
   item.title=NewsField(item.title,300).Replace("\r"," ").Replace("\n"," ");
   item.summary=NewsField(item.summary,12000);item.significance=NewsField(item.significance,4000);
   item.url=NewsUrl(item.url);item.source=String.IsNullOrWhiteSpace(item.source)?"":NewsUrl(item.source);item.collector=agent;
   if(!newsCategories.Contains(item.category))item.category="その他";
  }
  MarkSharedSources(batch.items);
  return batch;
 }
 static readonly string newsFile=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"RustAIWorkspace","news.json");
 static NewsState LoadNews(){return LoadFeed(newsFile);}
 static NewsState LoadFeed(string path){
  if(File.Exists(path)){
   using(var file=new FileStream(path,FileMode.Open,FileAccess.Read,FileShare.ReadWrite|FileShare.Delete))
   using(var reader=new StreamReader(file)){
    var saved=new JavaScriptSerializer{MaxJsonLength=int.MaxValue}.Deserialize<NewsState>(reader.ReadToEnd());
    if(path==jobsFile)NormalizeJobs(saved);return saved;
   }
  }
  var state=new NewsState();if(path==jobsFile)state.sources=new string[0];return state;
 }
 static FileStream LockNews(string path){
  for(int attempt=0;;attempt++){
   try{return new FileStream(path+".lock",FileMode.OpenOrCreate,FileAccess.Write,FileShare.None);}
   catch(IOException){if(attempt>=30)throw;System.Threading.Thread.Sleep(30);}
  }
 }
 static NewsState UpdateNews(Action<NewsState> change){
  return UpdateFeed(newsFile,change);
 }
 static NewsState UpdateFeed(string path,Action<NewsState> change){
  Directory.CreateDirectory(Path.GetDirectoryName(path));
  using(LockNews(path)){
   var state=LoadFeed(path);
   if(path==jobsFile && state.jobFirstSeen==null){
    state.jobFirstSeen=new Dictionary<string,string>();
    foreach(var job in state.articles)state.jobFirstSeen[JobKey(job.url)]="";
    if(state.pinnedJobs!=null)foreach(var url in state.pinnedJobs.Keys)state.jobFirstSeen[url]="";
   }
   change(state);
   if(path==jobsFile){
    var today=JobsDay();
    foreach(var job in state.articles)
     if(!state.jobFirstSeen.ContainsKey(JobKey(job.url)))state.jobFirstSeen.Add(JobKey(job.url),today);
   }
   if(path==jobsFile && state.pinnedJobs!=null)
    foreach(var job in state.articles)
     if(state.pinnedJobs.ContainsKey(JobKey(job.url)))state.pinnedJobs[JobKey(job.url)]=job;
   var temp=path+"."+Guid.NewGuid().ToString("N");
   try{File.WriteAllText(temp,new JavaScriptSerializer{MaxJsonLength=int.MaxValue}.Serialize(state));
    if(File.Exists(path))File.Replace(temp,path,null);else File.Move(temp,path);
   }finally{if(File.Exists(temp))File.Delete(temp);}
   return state;
  }
 }
 Button newsButton;
 bool newsReady;
 void HideNews(){
  At<Border>("Jobs").Visibility=Visibility.Collapsed;
  System.Windows.Controls.Primitives.Selector.SetIsSelected(jobsButton,false);
  At<Border>("News").Visibility=Visibility.Collapsed;
  System.Windows.Controls.Primitives.Selector.SetIsSelected(newsButton,false);
 }
 void ShowNews(){
  HideNews();
  var host=At<Border>("News");
  if(!newsReady){
   try{BuildNews();newsReady=true;}catch(Exception ex){MessageBox.Show(window,ex.Message,"AIニュース");return;}
  }
  HideGuard();HideCulture();
  At<Grid>("Workspace").Visibility=Visibility.Collapsed;
  At<Border>("Analytics").Visibility=Visibility.Collapsed;
  foreach(var b in At<StackPanel>("Rail").Children.OfType<Button>())
   System.Windows.Controls.Primitives.Selector.SetIsSelected(b,b==newsButton);
  host.Visibility=Visibility.Visible;
 }
 static bool IsNewsSource(string value,string[] sources){
  Uri uri;
  if(!Uri.TryCreate(value,UriKind.Absolute,out uri))return false;
  string host=uri.IdnHost.TrimEnd('.');
  return host=="youtu.be" || host=="youtube.com" || host.EndsWith(".youtube.com",StringComparison.OrdinalIgnoreCase) ||
   sources.Any(source=>{Uri registered;return Uri.TryCreate(source,UriKind.Absolute,out registered) && registered.Equals(uri);});
 }
 static void RepairNewsEndpoints(NewsState state){
  foreach(var pair in state.endpoints.ToArray()){
   if(!IsNewsSource(pair.Value,state.sources))continue;
   var url=NewsUrl(pair.Value);
   if(!state.sources.Any(source=>new Uri(source).Equals(new Uri(url))))
    state.sources=state.sources.Concat(new[]{url}).ToArray();
   state.endpoints.Remove(pair.Key);
  }
 }
 void BuildNews(){
  NewsState state=LoadNews();
  if(state.endpoints.Values.Any(ep=>IsNewsSource(ep,state.sources)))state=UpdateNews(RepairNewsEndpoints);
  var root=At<DockPanel>("NewsRoot");
  var sources=At<TextBox>("NewsSources");sources.Text=String.Join(Environment.NewLine,state.sources);
  var agent=At<ComboBox>("NewsAgent");agent.ItemsSource=new[]{"dots","muse"};agent.SelectedIndex=0;
  var endpoint=At<TextBox>("NewsEndpoint");
  Action showEndpoint=delegate{string s;endpoint.Text=state.endpoints.TryGetValue((string)agent.SelectedItem,out s)?s:"";};
  agent.SelectionChanged+=delegate{showEndpoint();};showEndpoint();
  var status=At<TextBlock>("NewsStatus");
  InitPatrol(status);
  var list=At<ListBox>("NewsList");
  var save=At<Button>("NewsSave");var fetch=At<Button>("NewsFetch");var copy=At<Button>("NewsCopy");
  var paste=At<Button>("NewsPaste");var stopWaiting=At<Button>("NewsStop");
  Action updateFetch=delegate{
   string who=(string)agent.SelectedItem,name=who=="muse"?"Muse":"Dots";
   bool pending=state.Pending(who)!=null,api=!String.IsNullOrWhiteSpace(endpoint.Text);
   fetch.Content=pending?name+"の返信を確認":api?"収集する":name+"に収集を依頼（DM）";
   fetch.ToolTip=!api?"登録URLを"+name+"にSlackのDMで依頼し、返信を自動で取り込みます。":null;
   copy.Visibility=api?Visibility.Visible:Visibility.Collapsed;
   stopWaiting.Visibility=pending?Visibility.Visible:Visibility.Collapsed;
  };
  endpoint.TextChanged+=delegate{updateFetch();};agent.SelectionChanged+=delegate{updateFetch();};updateFetch();
  stopWaiting.Click+=delegate{
   if(MessageBox.Show(window,"この依頼の返信確認を終了しますか？ 送信済みのDMはSlackに残ります。","AIニュース",MessageBoxButton.YesNo)!=MessageBoxResult.Yes)return;
   try{state=UpdateNews(s=>s.SetPending((string)agent.SelectedItem,null));updateFetch();status.Text="返信の確認を終了しました。";}catch(Exception e){status.Text=e.Message;}
  };
  var open=At<Button>("NewsOpen");var task=At<Button>("NewsTask");
  var reader=At<DockPanel>("NewsArticle");
  var variant=At<ComboBox>("NewsVariant");
  Action showSource=delegate{
   var item=variant.SelectedItem as NewsItem;reader.DataContext=item;
   At<StackPanel>("NewsRegistered").Visibility=item!=null && !String.IsNullOrEmpty(item.source)?Visibility.Visible:Visibility.Collapsed;
   At<ScrollViewer>("NewsScroll").ScrollToTop();
  };
  variant.SelectionChanged+=delegate{showSource();};
  Action select=delegate{
   var i=list.SelectedItem as NewsItem;
   variant.ItemsSource=i==null?null:i.Sources;variant.SelectedIndex=i==null?-1:0;variant.Visibility=i!=null && i.Sources.Count>1?Visibility.Visible:Visibility.Collapsed;showSource();reader.Visibility=i==null?Visibility.Collapsed:Visibility.Visible;
   At<TextBlock>("NewsEmpty").Visibility=i==null?Visibility.Visible:Visibility.Collapsed;

   At<ScrollViewer>("NewsScroll").ScrollToTop();
  };
  var tabs=At<TabControl>("NewsTabs");
  var markTabRead=At<Button>("NewsMarkTabRead");
  const string all="すべて";
  foreach(string category in new[]{all}.Concat(newsCategories))tabs.Items.Add(new TabItem{Tag=category,Header="0"});
  tabs.SelectedIndex=0;
  var stories=new List<NewsItem>();
  bool updatingList=false;
  Func<NewsItem,string> categoryOf=i=>newsCategories.Contains(i.category)?i.category:"その他";
  Action updateReadButton=delegate{
   var tab=tabs.SelectedItem as TabItem;
   if(tab==null)return;
   string scope=(string)tab.Tag==all?"全分類":(string)tab.Tag;
   markTabRead.IsEnabled=tab.Header!=null;
   markTabRead.Content=tab.Header==null?"新着はありません":("新着 "+tab.Header+" 件を閲覧済みにする");
   markTabRead.ToolTip=scope+"のニュースが対象です";
  };
  Action updateBadges=delegate{
   var seen=new HashSet<string>(state.readNewsUrls);
   foreach(var story in stories)story.SetUnread(!story.Sources.Any(i=>seen.Contains(i.ReadKey)));
   var unread=stories.Where(story=>story.IsUnread).ToList();
   foreach(TabItem tab in tabs.Items){
    string category=(string)tab.Tag;
    int count=category==all?unread.Count:unread.Count(i=>categoryOf(i)==category);
    tab.Header=count==0?null:count.ToString();
    tab.ToolTip=category+"：新着 "+count+"件";
    System.Windows.Automation.AutomationProperties.SetName(tab,category+"、新着 "+count+"件");
   }
   updateReadButton();
  };
  const int pageSize=20;
  int pageIndex=0;
  var visible=new List<NewsItem>();
  var prevPage=At<Button>("NewsPrevPage");
  var nextPage=At<Button>("NewsNextPage");
  Action showPage=delegate{
   int lastPage=Math.Max(0,(visible.Count-1)/pageSize);
   pageIndex=Math.Max(0,Math.Min(pageIndex,lastPage));
   var page=visible.GetRange(pageIndex*pageSize,Math.Min(pageSize,visible.Count-pageIndex*pageSize));
   var previous=list.SelectedItem as NewsItem;
   updatingList=true;
   try{
    list.ItemsSource=page;
    list.SelectedItem=previous==null?null:page.Find(i=>i.Sources.Any(a=>a.ReadKey==previous.ReadKey));
    if(page.Count>0)list.ScrollIntoView(list.SelectedItem??page[0]);
    select();
   }finally{updatingList=false;}
   prevPage.IsEnabled=pageIndex>0;
   nextPage.IsEnabled=pageIndex<lastPage;
   At<TextBlock>("NewsPageNumber").Text=(pageIndex+1)+" / "+(lastPage+1);
   At<StackPanel>("NewsPager").Visibility=lastPage>0?Visibility.Visible:Visibility.Collapsed;
  };
  Action applyTab=delegate{
   var tab=tabs.SelectedItem as TabItem;if(tab==null)return;
   updateReadButton();
   string category=(string)tab.Tag;
   visible=category==all?stories:stories.Where(i=>categoryOf(i)==category).ToList();
    At<TextBlock>("NewsCount").Text=(category==all?"全分類":category)+"  ·  ニュース "+visible.Count+"件  ·  収集結果 "+visible.Sum(i=>i.Sources.Count)+"件";
    At<TextBlock>("NewsEmpty").Text=visible.Count==0?"この分類にはまだニュースがありません。":"一覧からニュースを選択してください。";
   showPage();
  };
  Action refresh=delegate{stories=NewsStories(state.articles);updateBadges();applyTab();};
  tabs.SelectionChanged+=delegate{pageIndex=0;applyTab();};
  prevPage.Click+=delegate{if(pageIndex>0){pageIndex--;showPage();}};
  nextPage.Click+=delegate{if(nextPage.IsEnabled){pageIndex++;showPage();}};
  Action<IEnumerable<NewsItem>> markRead=delegate(IEnumerable<NewsItem> items){
   var keys=items.SelectMany(i=>i.Sources).Select(i=>i.ReadKey).Distinct().ToList();
   if(keys.Count==0)return;
   try{
    state=UpdateNews(s=>{
     var seen=new HashSet<string>(s.readNewsUrls);
     foreach(var key in keys)if(seen.Add(key))s.readNewsUrls.Add(key);
    });
    updateBadges();
   }catch(Exception ex){status.Text="既読の保存に失敗しました。"+ex.Message;}
  };
  markTabRead.Click+=delegate{markRead(visible);};
  list.SelectionChanged+=delegate{
   if(updatingList)return;
   select();var story=list.SelectedItem as NewsItem;
   if(story!=null && !story.Sources.All(i=>state.readNewsUrls.Contains(i.ReadKey)))markRead(new[]{story});
  };
  refresh();
  Action persist=delegate{
   var urls=sources.Text.Split(new[]{'\r','\n'},StringSplitOptions.RemoveEmptyEntries).Where(x=>x.Trim().Length>0).Select(NewsUrl).Distinct().ToArray();
   var ep=endpoint.Text.Trim();if(ep!=""){ep=NewsUrl(ep);if(IsNewsSource(ep,urls))throw new FormatException("このURLは登録ソース欄に入力し、API接続先は空欄にしてください。");if(!ep.StartsWith("https://"))throw new FormatException("APIにはHTTPSを指定してください。");}
   state=UpdateNews(s=>{s.sources=urls;s.endpoints[(string)agent.SelectedItem]=ep;});
  };
  Action<string,string> ingest=(json,who)=>{
   var items=ReadNews(json,who);
   state=UpdateNews(s=>{foreach(var i in items.AsEnumerable().Reverse()){s.articles.RemoveAll(old=>old.ReadKey==i.ReadKey && old.collector==i.collector);s.articles.Insert(0,i);}});
   refresh();status.Text=items.Count+"件の収集結果を確認しました。";
  };
  save.Click+=delegate{try{persist();status.Text="設定を保存しました。";}catch(Exception e){status.Text=e.Message;}};
  Action copyRequest=delegate{
   string who=(string)agent.SelectedItem;
   Clipboard.SetText(NewsRequest(who,state.sources));
   status.Text=who+" への依頼をコピーしました。担当に貼り付け、返答をコピーして「結果JSONを取り込む」を押してください。";
  };
  copy.Click+=delegate{try{persist();copyRequest();}catch(Exception e){status.Text=e.Message;}};
  paste.Click+=delegate{try{ingest(Clipboard.GetText(),(string)agent.SelectedItem);}catch(Exception e){status.Text=e.Message;}};
  var checking=new HashSet<string>();
  var statuses=new Dictionary<string,string>();
  Action<string,string> report=(who,msg)=>{statuses[who]=msg;status.Text=String.Join("　／　",new[]{"dots","muse"}.Where(statuses.ContainsKey).Select(k=>(k=="muse"?"Muse":"Dots")+" "+statuses[k]));};
  Func<string,System.Threading.Tasks.Task> checkReply=async who=>{
   var pending=state.Pending(who);
   if(pending==null || !checking.Add(who))return;
   try{
    var response=await SlackNewsCall("read",new {pending.id,pending.channel,pending.ts,collector=who});
    if(state.Pending(who)==null || state.Pending(who).id!=pending.id)return;
    var replies=response.replies??new string[0];
    string partial;
    var items=CollectReply(replies,who,out partial);
    if(items!=null){
     state=UpdateNews(s=>{
      if(s.Pending(who)==null || s.Pending(who).id!=pending.id)return;
      foreach(var article in items.AsEnumerable().Reverse()){s.articles.RemoveAll(old=>old.ReadKey==article.ReadKey && old.collector==article.collector);s.articles.Insert(0,article);}
      s.SetPending(who,null);
     });
     refresh();updateFetch();
     report(who,items.Count==0?"収集完了（該当0件）":items.Count+"件を取り込みました。");return;
    }
    if(partial!=null){report(who,partial);return;}
    report(who,(replies.Length==0?"DM返信なし・開始未確認":"返信あり・記事一覧待ち")+" "+DateTime.Now.ToString("HH:mm:ss"));
   }catch(Exception e){report(who,e.Message);}finally{checking.Remove(who);}
  };
  bool pollingPatrol=false;
  DateTime lastDiskChange=File.GetLastWriteTimeUtc(newsFile);
  Func<System.Threading.Tasks.Task> checkPatrol=async delegate{
   if(pollingPatrol)return;
   pollingPatrol=true;
   try{
    var snapshot=LoadNews();
    foreach(var entry in snapshot.patrolReceipts){
     try{await ReceivePatrol(entry.Key,entry.Value);}catch(Exception ex){report(PatrolWho(entry.Key),ex.Message);}
    }
    var modified=File.GetLastWriteTimeUtc(newsFile);
    if(modified!=lastDiskChange){state=LoadNews();refresh();updateFetch();lastDiskChange=modified;}
    if(state.patrolReceipts.Count>0 && state.pendingDots==null && state.pendingMuse==null)
     status.Text="定期巡回の返信待ち "+state.patrolReceipts.Count+"件";
   }catch(Exception ex){status.Text=ex.Message;}finally{pollingPatrol=false;}
  };
  var timer=new System.Windows.Threading.DispatcherTimer{Interval=TimeSpan.FromSeconds(65)};
  timer.Tick+=async delegate{if(root.IsVisible)await System.Threading.Tasks.Task.WhenAll(checkReply("dots"),checkReply("muse"),checkPatrol());};
  root.IsVisibleChanged+=async delegate{if(root.IsVisible){timer.Start();await System.Threading.Tasks.Task.WhenAll(checkReply("dots"),checkReply("muse"),checkPatrol());}else timer.Stop();};
  fetch.Click+=async delegate{
   try{persist();string who=(string)agent.SelectedItem;string ep=state.endpoints[who];
    if(state.Pending(who)!=null){await checkReply(who);return;}
    if(ep==""){
     root.IsEnabled=false;
     var id=Guid.NewGuid().ToString("N");
     var request="KAGARI AIニュース収集依頼 "+id+"。このDMのスレッドに結果JSONを返信してください。スレッド外の場合は依頼IDを添えてください。\n"+NewsRequest(who,state.sources);
     status.Text=who+"にDMを送信しています…";
     var receipt=await SlackNewsCall("send",new {prompt=request,collector=who});receipt.id=id;
     state=UpdateNews(s=>s.SetPending(who,receipt));updateFetch();report(who,"DM送信済み・開始未確認");return;
    }
    root.IsEnabled=false;status.Text=who+" が収集中…";
    using(var http=new HttpClient(new HttpClientHandler{AllowAutoRedirect=false}){Timeout=TimeSpan.FromMinutes(3),MaxResponseContentBufferSize=1000000})
    using(var body=new StringContent(NewsRequest(who,state.sources),Encoding.UTF8,"application/json"))
    using(var reply=await http.PostAsync(ep,body)){reply.EnsureSuccessStatusCode();ingest(await reply.Content.ReadAsStringAsync(),who);}
   }catch(Exception e){status.Text=e.Message;}finally{root.IsEnabled=true;}
  };
  Action<string> openLink=delegate(string address){
   try{System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(NewsUrl(address)){UseShellExecute=true});}
   catch(Exception e){status.Text=e.Message;}
  };
  reader.AddHandler(System.Windows.Documents.Hyperlink.RequestNavigateEvent,new System.Windows.Navigation.RequestNavigateEventHandler(delegate(object sender,System.Windows.Navigation.RequestNavigateEventArgs e){
   e.Handled=true;
   openLink(e.Uri.OriginalString);
  }));
  open.Click+=delegate{var item=variant.SelectedItem as NewsItem;if(item!=null)openLink(item.url);};
  task.Click+=delegate{var i=(NewsItem)list.SelectedItem;string note=String.Join("\n\n",i.Sources.Select(a=>a.summary+"\n\n重要な理由\n"+a.significance+"\n\n原文　"+a.url+(String.IsNullOrEmpty(a.source)?"":"\n登録ソース　"+a.source)+"\n収集担当　"+a.collector));Run(747,0,i.title+"\n"+note);};

 }
}
