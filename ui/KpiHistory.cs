using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.Serialization.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shapes;
using Path=System.IO.Path;

internal sealed partial class Desktop {
 readonly List<KpiSnapshot> observations=new List<KpiSnapshot>();
 static readonly string[] measures={"課題あり","確認待ち","作業中","完了","総件数","完了率"};
 static readonly string[] trendColors={"#BF4939","#A76B05","#2876A1","#20815A","#344958","#7854AA"};
 string HistoryDirectory {get{return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"RustAIWorkspace","kpi-history");}}
 void InitKpiHistory(){
  var points=At<ComboBox>("KpiPoint");
  var baseline=At<ComboBox>("KpiBaseline");
  baseline.Items.Add("直前の保存時点");
  points.Items.Add("最新の集計");
  var unreadable=new List<string>();
  try{
   if(Directory.Exists(HistoryDirectory))foreach(string file in Directory.GetFiles(HistoryDirectory,"*.json")){
    try{using(var input=File.OpenRead(file)){
     var point=(KpiSnapshot)new DataContractJsonSerializer(typeof(KpiSnapshot)).ReadObject(input);
     if(point==null || point.Received==default(DateTime) || point.All==null || point.Projects==null || point.Agents==null)throw new InvalidDataException();
     observations.Add(point);
    }}catch(Exception e){unreadable.Add(Path.GetFileName(file)+"　"+e.Message);}
   }
  }catch(IOException e){unreadable.Add(HistoryDirectory+"　"+e.Message);}catch(UnauthorizedAccessException e){unreadable.Add(HistoryDirectory+"　"+e.Message);}
  if(unreadable.Count>0)At<TextBlock>("KpiHistoryNotice").Text="読み込めない保存記録\n"+string.Join("\n",unreadable);
  observations.Sort((a,b)=>a.Received.CompareTo(b.Received));
  foreach(var point in observations.AsEnumerable().Reverse()){
   points.Items.Add(point);
   baseline.Items.Add(point);
  }
  baseline.SelectedIndex=0;
  points.SelectedIndex=0;
  At<ComboBox>("GoalMetric").ItemsSource=measures;
  At<ComboBox>("GoalMetric").SelectedItem="確認待ち";
  At<ComboBox>("GoalScope").ItemsSource=new[]{"全体"};
  At<ComboBox>("GoalScope").SelectedIndex=0;
  var legend=At<WrapPanel>("KpiTrendLegend");
  for(int i=0;i<measures.Length;i++){
   var key=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(6,0,8,0)};
   var sample=new Line{X1=0,X2=22,Y1=5,Y2=5,Height=10,Width=24,Stroke=Color(trendColors[i]),StrokeThickness=2.5,VerticalAlignment=VerticalAlignment.Center};
   sample.StrokeDashArray=TrendDash(i);
   key.Children.Add(sample);key.Children.Add(Label(measures[i]+(i==5?"（右軸）":""),14));legend.Children.Add(key);
  }
  legend.Children.Add(Label("◆ 改善策の記録",14));
  At<ComboBox>("KpiObservation").SelectionChanged+=(s,e)=>ShowObservation();
  points.SelectionChanged+=(s,e)=>RenderKpiPoint();
  baseline.SelectionChanged+=(s,e)=>DrawKpiTrend();
  At<ComboBox>("KpiScope").SelectionChanged+=(s,e)=>DrawKpiTrend();
  At<Canvas>("KpiPlot").SizeChanged+=(s,e)=>DrawKpiTrend();
  At<Button>("KpiSave").Click+=(s,e)=>LoadAnalytics(true);
 }
 void SaveKpiPoint(KpiSnapshot point){
  var memo=At<TextBox>("KpiMemo");
  KpiGoal goal=null;
  if(At<CheckBox>("KpiGoalEditor").IsChecked==true){
   var scope=At<ComboBox>("GoalScope").SelectedItem as string;
   int metric=At<ComboBox>("GoalMetric").SelectedIndex;
   var date=At<DatePicker>("GoalDate").SelectedDate;
   double target;
   var baseline=MeasureValue(ScopeRow(point,scope),metric);
   string error=null;
   if(!baseline.HasValue)error="選んだ範囲・指標の基準値を取得できません。対象を変更してください。";
   else if(!double.TryParse(At<TextBox>("GoalTarget").Text,NumberStyles.AllowDecimalPoint,CultureInfo.CurrentCulture,out target)
     || double.IsNaN(target) || target<0 || (metric==5?target>100:target>int.MaxValue || target!=Math.Truncate(target)))
    error=metric==5?"完了率の目標値は0～100の数値で入力してください。":"件数の目標値は0以上の整数で入力してください。";
   else if(!date.HasValue || date.Value.Date<point.Received.Date)error="次回確認日には今日以降の日付を指定してください。";
   else goal=new KpiGoal{Scope=scope,Metric=measures[metric],Baseline=baseline.Value,Target=target,
    AtLeast=At<ComboBox>("GoalDirection").SelectedIndex==1,CheckDate=date.Value.ToString("yyyy-MM-dd",CultureInfo.InvariantCulture)};
   if(error!=null){At<TextBlock>("KpiHistoryNotice").Text=error;return;}
  }
  point.Memo=memo.Text.Trim();
  point.Goal=goal;
  string dir=HistoryDirectory, temp=null;
  try{
   Directory.CreateDirectory(dir);
   var path=Path.Combine(dir,Guid.NewGuid().ToString("N")+".json");
   temp=path+".tmp";
   using(var output=new FileStream(temp,FileMode.CreateNew,FileAccess.Write,FileShare.None)){
    new DataContractJsonSerializer(typeof(KpiSnapshot)).WriteObject(output,point);
    output.Flush(true);
   }
   File.Move(temp,path);
  }catch(Exception){
   point.Memo=null;point.Goal=null;
   At<TextBlock>("KpiHistoryNotice").Text="記録を保存できませんでした。空き容量と保存先のアクセス権を確認してください。";
   return;
  }finally{if(temp!=null && File.Exists(temp)){try{File.Delete(temp);}catch(IOException){}}}
  observations.Add(point);observations.Sort((a,b)=>a.Received.CompareTo(b.Received));
  At<ComboBox>("KpiPoint").Items.Insert(1,point);
  At<ComboBox>("KpiBaseline").Items.Insert(1,point);
  memo.Clear();
  At<CheckBox>("KpiGoalEditor").IsChecked=false;
  At<TextBox>("GoalTarget").Clear();
  At<DatePicker>("GoalDate").SelectedDate=null;
  At<TextBlock>("KpiHistoryNotice").Text=point+" の集計を保存しました。";
 }
 static double? MeasureValue(KpiRow r,int i){
  if(r==null)return null;
  switch(i){case 0:return r.Issue;case 1:return r.Waiting;case 2:return r.Active;case 3:return r.Done;case 4:return r.Total;default:return r.Total==0?(double?)null:100.0*r.Done/r.Total;}
 }
 const string ProjectPrefix="プロジェクト　", AgentPrefix="担当AI　";
 static KpiRow ScopeRow(KpiSnapshot p,string scope){
  if(scope=="全体")return p.All;
  bool project=scope.StartsWith(ProjectPrefix,StringComparison.Ordinal);
  KpiRow row;
  return (project?p.Projects:p.Agents).TryGetValue(scope.Substring(project?ProjectPrefix.Length:AgentPrefix.Length),out row)?row:null;
 }
 void UpdateKpiScopes(KpiSnapshot current){
  var box=At<ComboBox>("KpiScope");var selection=box.SelectedItem as string;
  var points=observations.Concat(new[]{current});
  var names=new[]{"全体"}.Concat(points.SelectMany(p=>p.Projects.Keys).Distinct().OrderBy(k=>k).Select(k=>ProjectPrefix+k))
    .Concat(points.SelectMany(p=>p.Agents.Keys).Distinct().OrderBy(k=>k).Select(k=>AgentPrefix+k)).ToArray();
  if(reviewPoint!=null && !names.Contains(reviewPoint.Goal.Scope))names=names.Concat(new[]{reviewPoint.Goal.Scope}).ToArray();
  if(!box.Items.Cast<string>().SequenceEqual(names)){
   box.ItemsSource=names;box.SelectedItem=names.Contains(selection)?selection:"全体";
  }
  if(analyticsData!=null){
   var editor=At<ComboBox>("GoalScope");
   var available=new[]{"全体"}.Concat(analyticsData.Projects.Keys.OrderBy(k=>k).Select(k=>ProjectPrefix+k))
    .Concat(analyticsData.Agents.Keys.OrderBy(k=>k).Select(k=>AgentPrefix+k)).ToArray();
   if(!editor.Items.Cast<string>().SequenceEqual(available)){
    var chosen=editor.SelectedItem as string;
    editor.ItemsSource=available;editor.SelectedItem=available.Contains(chosen)?chosen:"全体";
   }
  }
 }
 static string GoalReview(KpiSnapshot saved,KpiSnapshot end){
  var goal=saved.Goal;
  int metric=Array.IndexOf(measures,goal.Metric);
  if(metric<0 || String.IsNullOrEmpty(goal.Scope) || !(goal.Scope=="全体" || goal.Scope.StartsWith(ProjectPrefix) || goal.Scope.StartsWith(AgentPrefix)))return "";
  bool later=end.Received>saved.Received;
  var actual=later?MeasureValue(ScopeRow(end,goal.Scope),metric):null;
  string unit=metric==5?"%":"件";
  return saved+"　"+goal.Scope+"／"+goal.Metric+"　次回確認日 "+goal.CheckDate+
   (String.IsNullOrEmpty(saved.Memo)?"":"　"+saved.Memo)+"\n基準値 "+goal.Baseline.ToString("0.##")+unit+
   "　目標値 "+goal.Target.ToString("G")+unit+(goal.AtLeast?"以上":"以下")+"　実績値 "+
   (actual.HasValue?actual.Value.ToString("0.##")+unit:later?"集計なし":"次回取得後に表示");
 }
 static string Delta(double value){return value.ToString("+0.#;-0.#;0",CultureInfo.CurrentCulture);}
 KpiSnapshot reviewPoint;
 string reviewTask;
 static bool GoalReached(KpiGoal goal,double value){
  return goal.AtLeast?value>=goal.Target:value<=goal.Target;
 }
 string EffectText(KpiSnapshot end){
  var goal=reviewPoint.Goal;
  int metric=Array.IndexOf(measures,goal.Metric);
  var actual=end==null?null:MeasureValue(ScopeRow(end,goal.Scope),metric);
  string result="未判定";
  if(end==null || end.Received.ToUniversalTime().Ticks/TimeSpan.TicksPerMillisecond<=reviewPoint.Received.ToUniversalTime().Ticks/TimeSpan.TicksPerMillisecond)result+="（基準時点より後の記録が必要）";
  else if(!actual.HasValue)result+="（同じ範囲の実績なし）";
  else if(String.CompareOrdinal(end.Received.ToLocalTime().ToString("yyyy-MM-dd",CultureInfo.InvariantCulture),goal.CheckDate)<0)result+="（確認日前）";
  else result=GoalReached(goal,actual.Value)?"達成":"未達成";
  return reviewTask+"　目標達成状況　"+result+"\n"+GoalReview(reviewPoint,end??reviewPoint);
 }
 void DrawGoalActions(KpiSnapshot end,string scope){
  At<TextBlock>("KpiEffect").Text=reviewPoint!=null && scope==reviewPoint.Goal.Scope?EffectText(end):"";
  var panel=At<StackPanel>("KpiTaskActions");panel.Children.Clear();
  foreach(var saved in observations.AsEnumerable().Reverse().Where(p=>p.Received<=end.Received && p.Goal!=null && (scope=="全体" || p.Goal.Scope==scope))){
   string detail=GoalReview(saved,end);
   if(detail.Length==0)continue;
   panel.Children.Add(new TextBlock{Text=detail,TextWrapping=TextWrapping.Wrap,Margin=new Thickness(0,8,0,2)});
   var create=new Button{Content="この改善策からタスクを起票",HorizontalAlignment=HorizontalAlignment.Left};
   create.Click+=(s,e)=>CreateImprovement(saved);panel.Children.Add(create);
  }
 }
 void CreateImprovement(KpiSnapshot saved){
  var goal=saved.Goal;
  var paths=data.Elements("project").Select(p=>PathName(p.Value)).ToArray();
  var layout=new StackPanel{Margin=new Thickness(16)};
  layout.Children.Add(new TextBlock{Text=GoalReview(saved,saved),TextWrapping=TextWrapping.Wrap});
  layout.Children.Add(Label("登録先プロジェクト",14));
  var destination=new ComboBox{ItemsSource=paths};
  var matches=Enumerable.Range(0,paths.Length).Where(i=>String.Equals(ProjectPrefix+ProjectName(paths[i]),goal.Scope,StringComparison.OrdinalIgnoreCase)).ToArray();
  destination.SelectedIndex=matches.Length==1?matches[0]:goal.Scope.StartsWith(ProjectPrefix)?-1:(int)data.Attribute("project");
  layout.Children.Add(destination);layout.Children.Add(Label("タスク名",14));
  var title=new TextBox{Text=String.IsNullOrWhiteSpace(saved.Memo)?goal.Scope+" "+goal.Metric+"の改善":saved.Memo.Replace('\r',' ').Replace('\n',' '),MaxLength=300};
  layout.Children.Add(title);
  var submit=new Button{Content="未着手で登録",HorizontalAlignment=HorizontalAlignment.Right,Margin=new Thickness(0,10,0,0)};
  layout.Children.Add(submit);
  var dialog=new Window{Owner=window,Title="改善タスクの起票",Width=510,SizeToContent=SizeToContent.Height,WindowStartupLocation=WindowStartupLocation.CenterOwner,Content=layout};
  submit.Click+=(s,e)=>{
   if(destination.SelectedIndex<0 || String.IsNullOrWhiteSpace(title.Text)){MessageBox.Show(dialog,"登録先とタスク名を指定してください。");return;}
   var link=new KpiSnapshot{Goal=goal,Received=saved.Received,Memo=saved.Memo};
   using(var buffer=new MemoryStream()){
    new DataContractJsonSerializer(typeof(KpiSnapshot)).WriteObject(buffer,link);
    string input=title.Text.Trim()+"\n"+goal.CheckDate+"\n"+Convert.ToBase64String(buffer.ToArray())+"\n"+GoalReview(saved,saved);
    if(Run(763,destination.SelectedIndex,input)){dialog.DialogResult=true;}
   }
  };
  if(dialog.ShowDialog()==true)ShowWorkspace();
 }
 void OpenGoal(System.Xml.Linq.XElement task,string encoded){
  try{
   using(var stream=new MemoryStream(Convert.FromBase64String(encoded))){
    var saved=(KpiSnapshot)new DataContractJsonSerializer(typeof(KpiSnapshot)).ReadObject(stream);
    if(saved.Goal==null || GoalReview(saved,saved).Length==0)throw new InvalidDataException();
    reviewPoint=saved;
   }
  }catch(Exception){MessageBox.Show(window,"このタスクのKPI記録を読み込めません。");return;}
  reviewTask=Value(task,"title")+"　作業状態　"+states[(int)task.Attribute("state")];
  At<TabItem>("KpiTrendTab").IsSelected=true;
  At<ComboBox>("KpiPoint").SelectedIndex=analyticsData==null && observations.Count>0?1:0;
  UpdateKpiScopes(DisplayPoint??reviewPoint);
  At<ComboBox>("KpiScope").SelectedItem=reviewPoint.Goal.Scope;
  var baseline=observations.FirstOrDefault(p=>p.Received.ToUniversalTime().Ticks/TimeSpan.TicksPerMillisecond==reviewPoint.Received.ToUniversalTime().Ticks/TimeSpan.TicksPerMillisecond);
  if(baseline!=null)At<ComboBox>("KpiBaseline").SelectedItem=baseline;
  else At<ComboBox>("KpiBaseline").SelectedIndex=0;
  At<TextBlock>("KpiEffect").Text=EffectText(DisplayPoint);
  OpenAnalytics(true);
 }
 static string ImprovementSummary(List<KpiSnapshot> points,string scope){
  if(scope.StartsWith(AgentPrefix,StringComparison.Ordinal))
   return "プロジェクトの改善対象を確認するには、分析対象で全体またはプロジェクトを選択してください。";
  if(points.Count<2)return "改善対象の判定には2回以上の観測が必要です。";
  int last=points.Count-1;
  var current=points[last];
  var previous=points[last-1];
  var findings=new List<string>();
  int comparable=0;
  foreach(var project in current.Projects.OrderBy(p=>p.Key)){
   if(scope!="全体" && scope!=ProjectPrefix+project.Key)continue;
   KpiRow old;
   if(!previous.Projects.TryGetValue(project.Key,out old))continue;
   comparable++;
   var row=project.Value;
   var reasons=new List<string>();
   if(row.Issue>old.Issue){
    reasons.Add("課題増加（+"+(row.Issue-old.Issue)+"件）　"+previous+"　"+old.Issue+"/"+old.Total+"件 → "+current+"　"+row.Issue+"/"+row.Total+"件");
   }
   if(row.Waiting>0 && old.Waiting>0){
    int first=last-1;
    while(first>0){
     KpiRow earlier;
     if(!points[first-1].Projects.TryGetValue(project.Key,out earlier) || earlier.Waiting==0)break;
     first--;
    }
    var evidence=points.Skip(first).Select(p=>p.Projects[project.Key]).Select(r=>r.Waiting+"/"+r.Total+"件");
    reasons.Add("確認待ちが"+(last-first+1)+"回連続　"+points[first]+" ～ "+current+"　"+string.Join(" → ",evidence));
   }
   if(reasons.Count>0)findings.Add(project.Key+"\n　"+string.Join("\n　",reasons));
  }
  if(comparable==0)return "改善対象の判定に必要な、前回と今回の両方に記録があるプロジェクトがありません。";
  return findings.Count==0?"改善対象なし（比較した"+comparable+"プロジェクト）":
   "改善対象 "+findings.Count+"／"+comparable+"プロジェクト（各観測の件数／プロジェクト総件数）\n"+string.Join("\n",findings);
 }
 static string ObservationDetail(KpiSnapshot point,string scope){
  var row=ScopeRow(point,scope);
  var values=measures.Select((name,m)=>{
   var value=MeasureValue(row,m);
   return name+" "+(value.HasValue?value.Value.ToString("0.#")+(m==5?"%":"件"):"欠測");
  });
  var memo=String.IsNullOrWhiteSpace(point.Memo)?"":"\n改善策　"+point.Memo;
  return point+memo+"\n"+scope+"　"+string.Join("　",values);
 }
 void ShowObservation(){
  var point=At<ComboBox>("KpiObservation").SelectedItem as KpiSnapshot;
  var scope=At<ComboBox>("KpiScope").SelectedItem as string;
  At<TextBlock>("KpiObservationDetail").Text=point==null || scope==null?"":ObservationDetail(point,scope);
 }
 void DrawKpiTrend(){
  var end=DisplayPoint;
  var scope=At<ComboBox>("KpiScope").SelectedItem as string;
  if(end==null || scope==null)return;
  var points=observations.Where(p=>p!=end && p.Received<=end.Received).Concat(new[]{end}).ToList();
  var picker=At<ComboBox>("KpiObservation");
  if(!picker.Items.Cast<KpiSnapshot>().SequenceEqual(points)){
   var selection=picker.SelectedItem as KpiSnapshot;
   picker.ItemsSource=points;
   picker.SelectedItem=points.Contains(selection)?selection:end;
  }
  ShowObservation();
  var previous=At<ComboBox>("KpiBaseline").SelectedItem as KpiSnapshot ?? (points.Count>1?points[points.Count-2]:null);
  var rows=points.Select(p=>ScopeRow(p,scope)).ToArray();
  var baselineRow=previous==null?null:ScopeRow(previous,scope);
  var selected=rows[rows.Length-1];
  var changes=new List<string>();
  if(previous!=null)for(int m=0;m<measures.Length;m++){
   var old=MeasureValue(baselineRow,m);var now=MeasureValue(selected,m);
   changes.Add(measures[m]+" "+(old.HasValue && now.HasValue?Delta(now.Value-old.Value)+(m==5?"ポイント":"件"):"欠測"));
  }
  At<TextBlock>("KpiComparison").Text=previous==null?"直前の保存記録がありません。基準時点を選ぶと比較できます。":
   "基準 "+previous+" → 比較 "+end+"\n"+string.Join("　",changes);
  var issues=new List<string>();
  foreach(var pair in end.Projects.Where(k=>scope=="全体" || scope==ProjectPrefix+k.Key)){
   var r=pair.Value;KpiRow old=null;
   if(previous!=null)previous.Projects.TryGetValue(pair.Key,out old);
   if(old!=null && (old.Issue!=r.Issue || old.Waiting!=r.Waiting)){
    issues.Add(pair.Key+"　課題 "+old.Issue+" → "+r.Issue+"件　確認待ち "+old.Waiting+" → "+r.Waiting+"件");
   }
  }
  string focus=selected==null?"この時点の集計なし":"確認する項目　課題あり "+selected.Issue+"件、確認待ち "+selected.Waiting+"件";
  At<TextBlock>("KpiChanges").Text=focus+(issues.Count>0?"\n"+string.Join("\n",issues):"")+"\n\n"+ImprovementSummary(points,scope);
  var notes=new List<string>();
  if(previous!=null && !String.IsNullOrEmpty(previous.Memo))notes.Add("基準時点の改善策　"+previous.Memo);
  if(!String.IsNullOrEmpty(end.Memo))notes.Add("比較時点の改善策　"+end.Memo);
  At<TextBlock>("KpiNotes").Text=string.Join("\n",notes);
  DrawGoalActions(end,scope);
  var canvas=At<Canvas>("KpiPlot");canvas.Children.Clear();
  double left=48,top=42,right=canvas.ActualWidth-52,bottom=canvas.Height-30;
  if(right<=left)return;
  double max=Math.Max(1,rows.Where(r=>r!=null).Select(r=>r.Total).DefaultIfEmpty(0).Max());
  PlotLine(canvas,left,top,right,top,"#DDE8EA");PlotLine(canvas,left,bottom,right,bottom,"#9BAFB5");
  PlotText(canvas,max.ToString("0")+"件",0,top-9);PlotText(canvas,"0件",8,bottom-10);
  PlotText(canvas,"100%",right+6,top-9);PlotText(canvas,"0%",right+6,bottom-10);
  DateTime first=points[0].Received;
  double span=(end.Received-first).TotalSeconds;
  var tips=points.Select(p=>ObservationDetail(p,scope)).ToArray();
  var xs=points.Select(p=>span==0?left:left+(right-left)*(p.Received-first).TotalSeconds/span).ToArray();
  for(int i=0;i<points.Count;i++){
   var point=points[i];
   if(String.IsNullOrWhiteSpace(point.Memo))continue;
   var guide=PlotLine(canvas,xs[i],24,xs[i],bottom,"#9BAFB5");
   guide.StrokeThickness=1;guide.StrokeDashArray=new DoubleCollection(new double[]{2,4});
   var marker=new Button{Content="◆",Width=24,Height=24,MinWidth=0,MinHeight=0,Padding=new Thickness(0),
    Margin=new Thickness(0),Foreground=Color("#7854AA"),ToolTip=tips[i]};
   System.Windows.Automation.AutomationProperties.SetName(marker,"改善策　"+point+"　"+point.Memo);
   marker.Click+=(s,e)=>picker.SelectedItem=point;
   Canvas.SetLeft(marker,xs[i]-12);Canvas.SetTop(marker,0);canvas.Children.Add(marker);
  }
  for(int m=0;m<measures.Length;m++){
   Point? last=null;
   for(int i=0;i<points.Count;i++){
    var value=MeasureValue(rows[i],m);
    if(!value.HasValue){last=null;continue;}
    double x=xs[i];
    double y=bottom-(bottom-top)*value.Value/(m==5?100:max);
    if(last.HasValue){
     var segment=PlotLine(canvas,last.Value.X,last.Value.Y,x,y,trendColors[m]);
     segment.StrokeDashArray=TrendDash(m);
    }
    double size=11-m;
    var dot=new Ellipse{Fill=Color(trendColors[m]),Width=size,Height=size,ToolTip=tips[i]};
    Canvas.SetLeft(dot,x-size/2);Canvas.SetTop(dot,y-size/2);canvas.Children.Add(dot);last=new Point(x,y);
   }
  }
  PlotText(canvas,first.ToString("MM/dd HH:mm"),left-6,bottom+3);
  if(span>0)PlotText(canvas,end.Received.ToString("MM/dd HH:mm"),right-62,bottom+3);
 }
 static DoubleCollection TrendDash(int metric){
  if(metric==5)return new DoubleCollection(new double[]{4,3});
  if(metric==4)return new DoubleCollection(new double[]{1,3});
  return null;
 }
 static Line PlotLine(Canvas c,double x,double y,double x2,double y2,string color){
  var line=new Line{X1=x,Y1=y,X2=x2,Y2=y2,Stroke=Color(color),StrokeThickness=2};
  c.Children.Add(line);return line;
 }
 static void PlotText(Canvas c,string text,double x,double y){
  var label=new TextBlock{Text=text,FontSize=12,Foreground=Color("#526671")};Canvas.SetLeft(label,x);Canvas.SetTop(label,y);c.Children.Add(label);
 }
}
