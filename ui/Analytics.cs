using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Runtime.Serialization;
using System.Runtime.Serialization.Json;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Media;

internal sealed partial class Desktop {
 [DataContract] sealed class ProgressFeed {
  [DataMember(Name="projects", IsRequired=true)] public ProgressItem[] Projects;
 }
 [DataContract] sealed class ProgressItem {
  [DataMember(Name="id")] public string Id;
  [DataMember(Name="name")] public string Name;
  [DataMember(Name="department")] public string Department;
  [DataMember(Name="agent")] public string Agent;
  [DataMember(Name="status",IsRequired=true)] public string Status;
 }
 [DataContract] sealed class KpiRow {
  [DataMember] public string Name {get;set;}
  [DataMember] public int Active {get;set;}
  [DataMember] public int Issue {get;set;}
  [DataMember] public int Waiting {get;set;}
  [DataMember] public int Done {get;set;}
  [DataMember] public int Total {get;set;}
  public string RateLabel {get{return Total==0?"—":((double)Done/Total).ToString("P1");}}
  public void Count(string status){
   Total++;
   switch(status){case "in_progress":Active++;break;case "issue":Issue++;break;case "waiting":Waiting++;break;case "done":Done++;break;}
  }
 }
 [DataContract] sealed class KpiGoal {
  [DataMember] public string Scope,Metric;
  [DataMember] public double Baseline,Target;
  [DataMember] public bool AtLeast;
  [DataMember] public string CheckDate;
 }
 [DataContract] sealed class KpiSnapshot {
  [DataMember] public KpiRow All=new KpiRow();
  [DataMember] public Dictionary<string,KpiRow> Projects=new Dictionary<string,KpiRow>();
  [DataMember] public Dictionary<string,KpiRow> Agents=new Dictionary<string,KpiRow>();
  [DataMember] public DateTime Received;
  [DataMember] public string Memo;
  [DataMember(EmitDefaultValue=false)] public KpiGoal Goal;
  public override string ToString(){return Received.ToLocalTime().ToString("yyyy/MM/dd HH:mm:ss");}
  static void Count(Dictionary<string,KpiRow> groups,string name,string status){
   KpiRow row;
   if(!groups.TryGetValue(name,out row)){row=new KpiRow{Name=name};groups.Add(name,row);}
   row.Count(status);
  }
  public void Add(ProgressItem item){
   string project=item.Department;
   if(String.IsNullOrEmpty(project)){
    string text=(item.Id+" "+item.Name).ToLowerInvariant();
    project=text.Contains("chromarch")?"CHROMARCH":text.Contains("mebae")?"mebae":
     Regex.IsMatch(text,"bunker|土の下")?"土の下の30日":Regex.IsMatch(text,"seahouse|漂流日和")?"漂流日和":
     Regex.IsMatch(text,"giin|議員ノート|ayumi")?"議員ノート":"その他";
   }
   string agent=item.Agent;
   switch(agent){case "claude_code":agent="Claude（このPC）";break;case "dots":agent="Dots";break;case "muse":agent="Muse";break;}
   All.Count(item.Status);Count(Projects,project,item.Status);Count(Agents,String.IsNullOrEmpty(agent)?"未設定":agent,item.Status);
  }
 }
 Button analyticsButton;
 KpiSnapshot analyticsData;
 bool updatingAnalytics;
 static readonly HttpClient metricsClient=new HttpClient(new HttpClientHandler{AllowAutoRedirect=false}){Timeout=TimeSpan.FromSeconds(15)};
 static string SavedAnalyticsKey(){
  var path=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"progress","secrets","view.dpapi");
  if(!File.Exists(path))return "";
  var hex=File.ReadAllText(path).Trim();
  var bytes=new byte[hex.Length/2];
  for(int i=0;i<bytes.Length;i++)bytes[i]=Convert.ToByte(hex.Substring(i*2,2),16);
  var plain=ProtectedData.Unprotect(bytes,null,DataProtectionScope.CurrentUser);
  try{return Encoding.Unicode.GetString(plain);}finally{Array.Clear(plain,0,plain.Length);}
 }
 void InitAnalytics(){
  InitKpiHistory();
  At<Button>("AnalyticsRefresh").Click+=(s,e)=>LoadAnalytics();
  At<Button>("AnalyticsConnect").Click+=(s,e)=>LoadAnalytics();
  At<RadioButton>("AnalyticsProject").Checked+=(s,e)=>ShowAnalyticsPies();
  At<RadioButton>("AnalyticsAgent").Checked+=(s,e)=>ShowAnalyticsPies();
 }
 void ShowWorkspace(){
  HideNews();
  HideGuard();
  HideCulture();
  At<Border>("Analytics").Visibility=Visibility.Collapsed;
  At<Grid>("Workspace").Visibility=Visibility.Visible;
  Selector.SetIsSelected(analyticsButton,false);
 }
 void OpenAnalytics(bool reviewing=false){
  HideNews();
  HideGuard();
  HideCulture();
  if(!reviewing){reviewPoint=null;At<TextBlock>("KpiEffect").Text="";}
  foreach(var b in At<StackPanel>("Rail").Children.OfType<Button>())Selector.SetIsSelected(b,b==analyticsButton);
  At<Grid>("Workspace").Visibility=Visibility.Collapsed;
  At<Border>("Analytics").Visibility=Visibility.Visible;
  LoadAnalytics();
 }
 async void LoadAnalytics(bool record=false){
  if(window.Dispatcher.HasShutdownStarted)return;
  if(!window.Dispatcher.CheckAccess() || !(System.Threading.SynchronizationContext.Current is System.Windows.Threading.DispatcherSynchronizationContext)){
   window.Dispatcher.BeginInvoke(new Action(()=>LoadAnalytics(record)));return;
  }
  if(updatingAnalytics)return;
  updatingAnalytics=true;
  At<Button>("KpiSave").IsEnabled=false;
  var refresh=At<Button>("AnalyticsRefresh");refresh.IsEnabled=false;
  var status=At<TextBlock>("AnalyticsTime");status.Text="共有進捗を集計中…";
  try{
   var key=At<PasswordBox>("AnalyticsKey").Password;
   if(String.IsNullOrWhiteSpace(key)){
    try{key=SavedAnalyticsKey();}catch(CryptographicException){key="";}catch(FormatException){key="";}
   }
   if(String.IsNullOrWhiteSpace(key))throw new UnauthorizedAccessException();
   using(var request=new HttpRequestMessage(HttpMethod.Get,"https://progress.eda-edamap.workers.dev/api/projects")){
    request.Headers.Authorization=new AuthenticationHeaderValue("Bearer",key.Trim());
    using(var response=await metricsClient.SendAsync(request)){
     if(response.StatusCode==HttpStatusCode.Unauthorized || response.StatusCode==HttpStatusCode.Forbidden)throw new UnauthorizedAccessException();
     response.EnsureSuccessStatusCode();
     using(var stream=await response.Content.ReadAsStreamAsync()){
      var feed=(ProgressFeed)new DataContractJsonSerializer(typeof(ProgressFeed)).ReadObject(stream);
      if(feed.Projects==null)throw new InvalidDataException();
      var snapshot=new KpiSnapshot{Received=DateTime.Now};
      foreach(var item in feed.Projects)snapshot.Add(item);
      analyticsData=snapshot;
     }
    }
   }
   At<StackPanel>("AnalyticsLogin").Visibility=Visibility.Collapsed;
   if(record)SaveKpiPoint(analyticsData);
   RenderKpiPoint();
  }catch(Exception err){
   var login=err is UnauthorizedAccessException;
   if(login)At<StackPanel>("AnalyticsLogin").Visibility=Visibility.Visible;
   status.Text=(login?"閲覧用合言葉を入力してください。":"共有進捗を取得できませんでした。更新して再試行してください。")+
    (analyticsData==null?"":"　表示中の取得日時 "+analyticsData.Received.ToString("yyyy/MM/dd HH:mm"));
  }finally{updatingAnalytics=false;refresh.IsEnabled=true;At<Button>("KpiSave").IsEnabled=true;}
 }
 KpiSnapshot DisplayPoint {get{return At<ComboBox>("KpiPoint").SelectedItem as KpiSnapshot ?? analyticsData;}}
 void RenderKpiPoint(){
   var point=DisplayPoint;
   if(point==null){
    At<StackPanel>("KpiTaskActions").Children.Clear();
    At<TextBlock>("KpiEffect").Text=reviewPoint==null?"":EffectText(null);
    At<WrapPanel>("AnalyticsMetrics").Children.Clear();
    At<WrapPanel>("AnalyticsPies").Children.Clear();
    At<StackPanel>("AnalyticsProjects").Children.Clear();
    At<WrapPanel>("AnalyticsLegend").Children.Clear();
    At<Canvas>("KpiPlot").Children.Clear();
    At<ComboBox>("KpiObservation").ItemsSource=null;
    At<TextBlock>("KpiObservationDetail").Text="";
    foreach(string name in new[]{"KpiComparison","KpiChanges","KpiNotes","KpiGoalReview"})At<TextBlock>(name).Text="";
    At<TextBlock>("AnalyticsTime").Text="最新の集計を取得してください。";
    return;
   }
   At<TextBlock>("AnalyticsTime").Text=(At<ComboBox>("KpiPoint").SelectedIndex>0?"保存した集計　":"共有進捗　取得 ")+point.ToString();
   var metrics=At<WrapPanel>("AnalyticsMetrics");metrics.Children.Clear();
   var all=point.All;
   AddKpi(metrics,"総件数",all.Total.ToString());AddKpi(metrics,"作業中",all.Active.ToString());
   AddKpi(metrics,"課題あり",all.Issue.ToString());AddKpi(metrics,"確認待ち",all.Waiting.ToString());
   AddKpi(metrics,"完了",all.Done.ToString());AddKpi(metrics,"完了率",all.RateLabel);
   ShowAnalyticsPies();
   UpdateKpiScopes(point);DrawKpiTrend();
 }
 static void AddKpi(Panel panel,string label,string value){
  var stack=new StackPanel();
  stack.Children.Add(Label(label,14));
  var number=Label(value,26);number.FontWeight=FontWeights.SemiBold;stack.Children.Add(number);
  panel.Children.Add(new Border{Child=stack,Background=Color("#E2F1F0"),CornerRadius=new CornerRadius(6),MinWidth=105,Padding=new Thickness(14,8,14,8),Margin=new Thickness(0,0,6,6)});
 }
 static void AddAssessmentLine(Panel panel,string label,string value){
  var line=new TextBlock{TextWrapping=TextWrapping.Wrap,FontSize=15,Margin=new Thickness(0,2,0,2)};
  line.Inlines.Add(new System.Windows.Documents.Run(label+"　"){FontWeight=FontWeights.SemiBold});
  line.Inlines.Add(value);panel.Children.Add(line);
 }
 void ShowAnalyticsPies(){
  var point=DisplayPoint;if(point==null)return;
  bool byAgent=At<RadioButton>("AnalyticsAgent").IsChecked==true;
  var projects=At<StackPanel>("AnalyticsProjects");
  var agents=At<WrapPanel>("AnalyticsPies");
  projects.Visibility=Visible(!byAgent);
  agents.Visibility=Visible(byAgent);
  Panel panel=byAgent?(Panel)agents:projects;
  var legend=At<WrapPanel>("AnalyticsLegend");
  panel.Children.Clear();legend.Children.Clear();
  string[] labels={"作業中","課題あり","確認待ち","完了","その他の状態"};
  string[] palette={"#327B9F","#C65742","#CF952B","#268367","#98A4AA"};
  var rows=(byAgent?point.Agents:point.Projects).Values.OrderByDescending(r=>r.Total).ThenBy(r=>r.Name).ToArray();
  if(rows.Length==0){panel.Children.Add(Label("この時点の集計は0件です",15));return;}
  bool unknown=rows.Any(r=>r.Total>r.Active+r.Issue+r.Waiting+r.Done);
  for(int i=0;i<(unknown?5:4);i++){
   var item=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(0,0,16,0)};
   item.Children.Add(new System.Windows.Shapes.Ellipse{Width=11,Height=11,Fill=Color(palette[i]),Margin=new Thickness(0,0,5,0)});
   item.Children.Add(Label(labels[i],14));legend.Children.Add(item);
  }
  foreach(var r in rows){
   int[] counts={r.Active,r.Issue,r.Waiting,r.Done,r.Total-r.Active-r.Issue-r.Waiting-r.Done};
   var pie=StatusPie(counts,palette);pie.Width=pie.Height=54;
   var content=new StackPanel{Orientation=Orientation.Horizontal};
   content.Children.Add(pie);
   var text=new StackPanel{Margin=new Thickness(12,0,0,0),Width=158,VerticalAlignment=VerticalAlignment.Center};
   var title=Label(r.Name,16);title.FontWeight=FontWeights.SemiBold;text.Children.Add(title);
   text.Children.Add(Label(r.Total+"件　完了率 "+r.RateLabel,14));content.Children.Add(text);
   var detail=String.Join("、",labels.Select((label,i)=>label+" "+counts[i]+"件").Take(unknown?5:4));
   var card=new Border{Child=content,ToolTip=r.Name+"\n"+detail,Padding=new Thickness(10),Margin=new Thickness(0,0,8,8),Background=Color("#F3F8F8"),CornerRadius=new CornerRadius(6)};
   System.Windows.Automation.AutomationProperties.SetName(pie,r.Name+"　"+detail);
   if(!byAgent){
    content.Orientation=Orientation.Vertical;
    content.Width=126;
    text.Margin=new Thickness(0,4,0,0);text.Width=126;
    title.TextWrapping=TextWrapping.Wrap;title.TextAlignment=TextAlignment.Center;
    ((TextBlock)text.Children[1]).TextAlignment=TextAlignment.Center;
    var row=new DockPanel();
    card.Child=null;
    DockPanel.SetDock(content,Dock.Left);row.Children.Add(content);
    var analysis=new StackPanel{Margin=new Thickness(14,0,2,0),VerticalAlignment=VerticalAlignment.Center};
    var status=new List<string>();
    for(int i=0;i<counts.Length;i++)if(counts[i]>0)status.Add(labels[i]+counts[i]+"件");
    string assessment=r.Total==0?"集計対象は0件です。":r.Done==r.Total?"全件完了です。":(r.Issue>0?"課題への対応が必要です。":r.Waiting>0?"判断を待つタスクがあります。":counts[4]>0?"状態の内訳に未分類があります。":"作業が進行中です。")+" "+String.Join("、",status)+"。";
    string action=r.Total==0?"対象タスクが登録されているか確認する。":
     r.Issue>0?"課題のある"+r.Issue+"件で、解消を妨げている点と対応担当を確認する。":
     r.Waiting>0?"確認待ちの"+r.Waiting+"件の成果を確認し、完了か修正依頼かを判断する。":
     counts[4]>0?"その他の状態に分類された"+counts[4]+"件の登録状態を確認する。":
     r.Active>0?"作業中の"+r.Active+"件について、次の成果物と完了条件を確認する。":
     "完了した成果を振り返り、次のタスクが必要か判断する。";
    if(r.Issue>0 && r.Waiting>0)action+=" 確認待ちの"+r.Waiting+"件はレビュー担当を決める。";
    AddAssessmentLine(analysis,"現状評価",assessment);
    AddAssessmentLine(analysis,"今後の対応",action);
    row.Children.Add(analysis);card.Child=row;
   }
   panel.Children.Add(card);
  }
 }
}

