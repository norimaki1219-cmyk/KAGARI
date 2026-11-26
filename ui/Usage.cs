using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Threading;
using System.Windows.Media;
using System.Windows.Shapes;
using System.Windows.Automation;
using Path = System.IO.Path;

internal sealed partial class Desktop {
 sealed class UsageWindow {
  public string Name;
  public double Percent;
  public DateTime Sample, Reset;
  public double Minutes;
  public bool Expired(DateTime now){return Reset==DateTime.MinValue?now>=Sample.AddMinutes(Minutes):now>=Reset;}
  public string Forecast(DateTime now,out string glyph,out string value){
   glyph="\uE823";value="";
   if(Expired(now))return "更新待ち";
   if(Reset==DateTime.MinValue)return "枠の開始待ち";
   if(Percent>=100){glyph="\uE7BA";return "上限到達 · "+FormatUsageTime(Reset)+" 解除予定";}
   if(Percent<=0 || Minutes<=0)return "予測待ち";
   double elapsed=Minutes-(Reset-Sample).TotalMinutes;
   if(elapsed<=0 || Sample>=Reset)return "予測待ち";
   double remaining=elapsed*(100-Percent)/Percent;
   if(remaining>=(Reset-Sample).TotalMinutes){glyph="\uE73E";return "枠内の見込み";}
   var eta=Sample.AddMinutes(remaining);
   if(eta<=now)return "予測更新待ち";
   glyph="\uE9D9";value=FormatUsageTime(eta);return "上限予測 "+value;
  }
 }
 static object Field(object value,string key){
  var obj=value as IDictionary<string,object>;object field;
  return obj!=null && obj.TryGetValue(key,out field)?field:null;
 }
 static double Number(object value){
  double number;
  return value!=null && Double.TryParse(Convert.ToString(value,CultureInfo.InvariantCulture),NumberStyles.Float,CultureInfo.InvariantCulture,out number)?number:Double.NaN;
 }
 static readonly DateTime epoch=new DateTime(1970,1,1,0,0,0,DateTimeKind.Utc);
 static string Period(double minutes){
  if(minutes%1440==0)return (minutes/1440)+"日枠";
  if(minutes%60==0)return (minutes/60)+"時間枠";
  return minutes+"分枠";
 }
 static FileStream OpenUsage(string path){return new FileStream(path,FileMode.Open,FileAccess.Read,FileShare.ReadWrite|FileShare.Delete);}
 static DateTime ParseStamp(object value){
  DateTimeOffset date;
  return DateTimeOffset.TryParse(value as string,CultureInfo.InvariantCulture,DateTimeStyles.AssumeUniversal,out date)?date.UtcDateTime:DateTime.MinValue;
 }
 struct UsageStamp {
  public long Time, Length;
  public UsageStamp(FileInfo file){
   Time=file.Exists?file.LastWriteTimeUtc.Ticks:0;
   Length=file.Exists?file.Length:-1;
  }
  public bool Matches(UsageStamp other){return Time==other.Time && Length==other.Length;}
 }
 sealed class CodexEntry {
  public UsageStamp Stamp;
  public Dictionary<string,UsageWindow> Limits;
 }
 static Dictionary<string,CodexEntry> codexFiles=new Dictionary<string,CodexEntry>();
 static Dictionary<string,UsageWindow> ReadCodexFile(FileInfo file,JavaScriptSerializer json){
  var latest=new Dictionary<string,UsageWindow>();
  using(var stream=OpenUsage(file.FullName)){
   long start=Math.Max(0,stream.Length-1024*1024);stream.Position=start;
   using(var reader=new StreamReader(stream)){
    if(start>0)reader.ReadLine();
    string line;
    while((line=reader.ReadLine())!=null){
     if(!line.Contains("\"rate_limits\"") || !line.Contains("\"token_count\""))continue;
     object record;try{record=json.DeserializeObject(line);}catch(ArgumentException){continue;}
     var payload=Field(record,"payload");
     if(!Equals(Field(payload,"type"),"token_count"))continue;
     var limits=Field(payload,"rate_limits");
     if(limits==null)continue;
     var stamp=ParseStamp(Field(record,"timestamp"));
     string id=Field(limits,"limit_id") as string??"codex";
     if(stamp==DateTime.MinValue)continue;
     foreach(string key in new[]{"primary","secondary"}){
      var slot=Field(limits,key);
      double usage=Number(Field(slot,"used_percent")),minutes=Number(Field(slot,"window_minutes")),reset=Number(Field(slot,"resets_at"));
      if(!(usage>=0 && usage<=100 && minutes>0 && reset>0 && reset<253402300799d))continue;
      string prefix=id=="codex"?"":(Field(limits,"limit_name") as string??id)+" ";
      string slotId=id+"/"+key;
      UsageWindow prior;
      if(latest.TryGetValue(slotId,out prior) && prior.Sample>=stamp)continue;
       latest[slotId]=new UsageWindow{Name=prefix+Period(minutes),Percent=usage,Minutes=minutes,Sample=stamp,Reset=epoch.AddSeconds(reset)};
     }
    }
   }
  }
  return latest;
 }
 static List<UsageWindow> CodexLimits(JavaScriptSerializer json){
  string home=Environment.GetEnvironmentVariable("CODEX_HOME")??Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),".codex");
  var root=new DirectoryInfo(Path.Combine(home,"sessions"));
  var files=new List<FileInfo>(25);
  if(root.Exists)foreach(var file in root.EnumerateFiles("rollout-*.jsonl",SearchOption.AllDirectories)){
   // Keep the original enumeration order when modification times tie.
   if(files.Count==24 && file.LastWriteTimeUtc<=files[23].LastWriteTimeUtc)continue;
   int i=files.Count;
   while(i>0 && files[i-1].LastWriteTimeUtc<file.LastWriteTimeUtc)i--;
   files.Insert(i,file);
   if(files.Count>24)files.RemoveAt(24);
  }
  var active=new Dictionary<string,CodexEntry>();
  var latest=new Dictionary<string,UsageWindow>();
  foreach(var file in files){
   try{
    var stamp=new UsageStamp(file);
    CodexEntry entry;
    if(!codexFiles.TryGetValue(file.FullName,out entry) || !entry.Stamp.Matches(stamp))
     entry=new CodexEntry{Stamp=stamp,Limits=ReadCodexFile(file,json)};
    active.Add(file.FullName,entry);
    foreach(var pair in entry.Limits){
     UsageWindow old;
     if(!latest.TryGetValue(pair.Key,out old) || old.Sample<pair.Value.Sample)latest[pair.Key]=pair.Value;
    }
   }catch(IOException){}catch(UnauthorizedAccessException){}
  }
  codexFiles=active;
  return latest.OrderBy(p=>p.Key).Select(p=>p.Value).ToList();
 }
 sealed class ClaudeRun {
  public UsageStamp Five, Seven, Error, Meta;
  public List<UsageWindow> Rows;
 }
 static Dictionary<string,ClaudeRun> claudeRuns=new Dictionary<string,ClaudeRun>();
 static List<UsageWindow> ClaudeLimits(JavaScriptSerializer json){
  var rows=LoadUsage(ClaudeCache)??new List<UsageWindow>();
  var root=new DirectoryInfo(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"RustAIWorkspace","history"));
  var active=new Dictionary<string,ClaudeRun>();
  var now=DateTime.UtcNow;
  if(root.Exists)foreach(var task in root.EnumerateDirectories()){
   foreach(var run in task.EnumerateDirectories()){
    if(run.LastWriteTimeUtc<now.AddDays(-8))continue;
    string path=run.FullName;
    try{
     var five=new UsageStamp(new FileInfo(Path.Combine(path,"usage-five_hour.json")));
     var seven=new UsageStamp(new FileInfo(Path.Combine(path,"usage-seven_day.json")));
     var error=new FileInfo(Path.Combine(path,"error.txt"));
     bool readError=error.Exists && error.Length<=2048 && error.LastWriteTimeUtc>=now.AddHours(-5);
     var err=readError?new UsageStamp(error):default(UsageStamp);
     var meta=readError?new UsageStamp(new FileInfo(Path.Combine(path,"meta.txt"))):default(UsageStamp);
     ClaudeRun entry;
     if(!claudeRuns.TryGetValue(path,out entry) || !entry.Five.Matches(five) || !entry.Seven.Matches(seven) || !entry.Error.Matches(err) || !entry.Meta.Matches(meta)){
      var samples=new List<UsageWindow>();
      try{ReadClaudeRun(path,json,samples,error,readError);}
      finally{ApplyClaudeSamples(rows,samples);}
      entry=new ClaudeRun{Five=five,Seven=seven,Error=err,Meta=meta,Rows=samples};
     }else ApplyClaudeSamples(rows,entry.Rows);
     active.Add(path,entry);
    }catch(IOException){}catch(UnauthorizedAccessException){}catch(ArgumentException){}
   }
  }
  claudeRuns=active;
  return rows;
 }
 static void ApplyClaudeSamples(List<UsageWindow> rows,List<UsageWindow> samples){
  foreach(var sample in samples)MergeUsage(rows,(int)sample.Minutes,sample.Percent,sample.Sample,sample.Reset);
 }
 static void ReadClaudeRun(string run,JavaScriptSerializer json,List<UsageWindow> rows,FileInfo error,bool readError){
  foreach(var key in new[]{"five_hour","seven_day"}){
   string file=Path.Combine(run,"usage-"+key+".json");
   if(!File.Exists(file))continue;
   object record;
   using(var reader=new StreamReader(OpenUsage(file)))record=json.DeserializeObject(reader.ReadToEnd());
   double pct=Number(Field(record,"percent")),reset=Number(Field(record,"reset")),sample=Number(Field(record,"sample"));
   if(pct>=0 && pct<=100 && reset>sample && reset<253402300799d && sample>0)
    rows.Add(new UsageWindow{Minutes=key=="five_hour"?300:10080,Percent=pct,Sample=epoch.AddSeconds(sample),Reset=epoch.AddSeconds(reset)});
  }
  // Older runs saved the CLI limit notice before structured rate-limit events were collected.

  if(!readError)return;
  string line;
  using(var reader=new StreamReader(OpenUsage(error.FullName)))line=reader.ReadLine();
  var match=System.Text.RegularExpressions.Regex.Match(line??"", @"^You've hit your 5-hour limit · resets (\d{1,2}:\d{2}[ap]m) \(Etc/GMT([+-]\d{1,2})?\)$",System.Text.RegularExpressions.RegexOptions.IgnoreCase);
  if(!match.Success)return;
  var meta=Path.Combine(run,"meta.txt");
  if(!File.Exists(meta) || !File.ReadAllText(meta).Contains("Claude Code /"))return;
  DateTime clock;
  if(!DateTime.TryParseExact(match.Groups[1].Value,"h:mmtt",CultureInfo.InvariantCulture,DateTimeStyles.None,out clock))return;
  int offset=match.Groups[2].Success?-Int32.Parse(match.Groups[2].Value,CultureInfo.InvariantCulture):0;
  var stamp=error.LastWriteTimeUtc;
  var resetUtc=stamp.AddHours(offset).Date.Add(clock.TimeOfDay).AddHours(-offset);
  if(resetUtc<=stamp)resetUtc=resetUtc.AddDays(1);
  if(resetUtc<=stamp.AddHours(5))rows.Add(new UsageWindow{Minutes=300,Percent=100,Sample=stamp,Reset=resetUtc});
 }
 static void MergeUsage(List<UsageWindow> rows,int minutes,double percent,DateTime sample,DateTime reset){
  var name=Period(minutes);
  var old=rows.Find(item=>item.Name==name);
  if(old!=null && old.Sample>=sample)return;
  if(old!=null)rows.Remove(old);
  rows.Add(new UsageWindow{Name=name,Minutes=minutes,Percent=percent,Sample=sample,Reset=reset});
 }
 static string configPath;
 static UsageStamp configStamp;
 static List<UsageWindow> configRows;
 static List<UsageWindow> ClaudeCache(JavaScriptSerializer json){
  var custom=Environment.GetEnvironmentVariable("CLAUDE_CONFIG_DIR");
  string path=Path.Combine(custom??Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),".claude.json");
  var stamp=new UsageStamp(new FileInfo(path));
  if(configPath==path && stamp.Matches(configStamp))return new List<UsageWindow>(configRows);
  var rows=ReadClaudeConfig(path,json);
  configPath=path;configStamp=stamp;configRows=rows;
  return new List<UsageWindow>(rows);
 }
 static List<UsageWindow> ReadClaudeConfig(string path,JavaScriptSerializer json){
  var rows=new List<UsageWindow>();
  if(!File.Exists(path))return rows;
  object cache;
  using(var reader=new StreamReader(OpenUsage(path)))cache=Field(json.DeserializeObject(reader.ReadToEnd()),"cachedUsageUtilization");
  var limits=Field(cache,"utilization") as IDictionary<string,object>;
  double ms=Number(Field(cache,"fetchedAtMs"));
  if(limits==null || !(ms>0 && ms<253402300799000d))return rows;
  foreach(var pair in limits.OrderBy(p=>p.Key)){
   double pct=Number(Field(pair.Value,"utilization"));
   var reset=ParseStamp(Field(pair.Value,"resets_at"));
   if(!(pct>=0 && pct<=100))continue;
   if(reset==DateTime.MinValue && !(pair.Key=="five_hour" && pct==0 && Field(pair.Value,"resets_at")==null))continue;
   double minutes=pair.Key=="five_hour"?300:pair.Key.StartsWith("seven_day",StringComparison.Ordinal)?10080:0;
   string name=minutes==0?pair.Key:Period(minutes);
   if(pair.Key.StartsWith("seven_day_"))name+=" · "+pair.Key.Substring(10).Replace('_',' ');
   rows.Add(new UsageWindow{Name=name,Percent=pct,Reset=reset,Minutes=minutes,Sample=epoch.AddMilliseconds(ms)});
  }
  return rows;
 }
 readonly DispatcherTimer usageTimer=new DispatcherTimer();
 [System.Runtime.InteropServices.DllImport("kagari_core")] static extern int kagari_refresh_claude_usage();
 readonly Dictionary<string,UsageWindow>[] priorUsage={new Dictionary<string,UsageWindow>(),new Dictionary<string,UsageWindow>()};
 DateTime manualAttempt;
 bool usageBusy;
 void StartUsage(){
  var refresh=At<Button>("UsageRefresh");
  refresh.Background=UsageFill("\uE72C");refresh.Foreground=UsageInk();
  var clock=(InlineUIContainer)window.FindName("UsageClock");
  clock.Child=UsageBadge("\uE823","最新取得");
  At<Button>("UsageRefresh").Click+=(s,e)=>RefreshUsage(true);
  usageTimer.Interval=TimeSpan.FromMinutes(1);
  usageTimer.Tick+=(s,e)=>RefreshUsage();
  usageTimer.Start();RefreshUsage();
 }
 async void RefreshUsage(bool manual=false){
  if(window.Dispatcher.HasShutdownStarted)return;
  if(!window.Dispatcher.CheckAccess() || !(System.Threading.SynchronizationContext.Current is System.Windows.Threading.DispatcherSynchronizationContext)){
   window.Dispatcher.BeginInvoke(new Action(()=>RefreshUsage(manual)));return;
  }
  if(usageBusy)return;
  usageBusy=true;
  var button=At<Button>("UsageRefresh");var message=At<Border>("UsageMessage");
  button.IsEnabled=false;
  try{
   int result=0;
   if(manual){
    manualAttempt=DateTime.UtcNow;button.Content="\uE823";UsageNotice(message,"\uE823","使用枠を照会中");
    result=await Task.Run(()=>kagari_refresh_claude_usage());
   }
   var groups=await Task.Run(()=>new[]{LoadUsage(CodexLimits),LoadUsage(ClaudeLimits)});
   if(!window.IsLoaded && core==IntPtr.Zero)return;
   var list=At<StackPanel>("UsageRows");list.Children.Clear();
   var now=DateTime.UtcNow;
   if(manual || manualAttempt!=DateTime.MinValue && groups[1]!=null && groups[1].Any(slot=>slot.Sample>=manualAttempt && !slot.Expired(now))){
    string glyph,description;
    DescribeUsageRefresh(result,groups[1],manualAttempt,now,out glyph,out description);
    UsageNotice(message,glyph,description);
   }
   var slots=new List<string>{Period(300),Period(10080)};
   for(int i=0;i<groups.Length;i++){
    if(groups[i]!=null)foreach(var slot in groups[i]){
     UsageWindow prior;
     if(!priorUsage[i].TryGetValue(slot.Name,out prior) || slot.Sample>=prior.Sample)priorUsage[i][slot.Name]=slot;
    }
    foreach(var name in priorUsage[i].Keys)if(!slots.Contains(name))slots.Add(name);
   }
   var latest=priorUsage.SelectMany(group=>group.Values).Select(slot=>slot.Sample).DefaultIfEmpty().Max();
   ((Run)window.FindName("UsageTimestamp")).Text=latest==DateTime.MinValue?"—":FormatUsageTime(latest);
   Grid.SetIsSharedSizeScope(list,true);
   for(int i=0;i<groups.Length;i++){
    var header=new StackPanel{Orientation=Orientation.Horizontal};
    string agent=i==0?"Codex":"Claude Code";
    header.Children.Add(new Border{Width=26,Height=26,CornerRadius=new CornerRadius(13),ToolTip=agent,
     Background=SystemParameters.HighContrast?SystemColors.ControlBrush:Color(i==0?"#DFE6EB":"#FBE2CF"),
     Child=AgentIcon(icons[i],18)});
    var agentName=Label(agent,15);
    agentName.FontWeight=FontWeights.SemiBold;
    agentName.Foreground=SystemParameters.HighContrast?SystemColors.ControlTextBrush:Color(i==0?"#23455F":"#743B25");
    agentName.Margin=new Thickness(6,0,4,0);
    header.Children.Add(agentName);
    AutomationProperties.SetName(header,agent);
    if(groups[i]==null)header.Children.Add(UsageLine("\uE7BA","","使用記録を読み込めません"));
    list.Children.Add(new Border{
     Child=header,CornerRadius=new CornerRadius(6),Padding=new Thickness(6,2,6,2),Margin=new Thickness(0,6,0,2),
     Background=SystemParameters.HighContrast?SystemColors.ControlBrush:Color(i==0?"#DBEAF5":"#FBE5D8"),
     BorderBrush=SystemParameters.HighContrast?SystemColors.ControlTextBrush:Color(i==0?"#7095AD":"#C7805C"),
     BorderThickness=SystemParameters.HighContrast?new Thickness(1):new Thickness(3,0,0,0)
    });
    var tiles=new Grid{HorizontalAlignment=HorizontalAlignment.Left,Margin=new Thickness(12,0,0,0)};
    for(int col=0;col<slots.Count;col++){
     tiles.ColumnDefinitions.Add(new ColumnDefinition{Width=GridLength.Auto,SharedSizeGroup="UsageColumn"+col});
     var cell=new StackPanel();
     UsageWindow slot;
     if(priorUsage[i].TryGetValue(slots[col],out slot)){
      bool fallback=groups[i]==null || !groups[i].Any(item=>item.Name==slot.Name && item.Sample==slot.Sample)
       || i==1 && slot.Sample<manualAttempt;
      cell.Children.Add(UsageTile(slot,now,fallback));
     }else if(col<2){
      cell.Margin=new Thickness(0,2,8,3);
      cell.Children.Add(UsageLine("\uE823",ShortPeriod(slots[col]),slots[col]+" · 取得待ち"));
      cell.Children.Add(UsageLine("\uE72C","—","リセット予定 不明"));
     }
     Grid.SetColumn(cell,col);tiles.Children.Add(cell);
    }
    list.Children.Add(tiles);
   }
  }finally{usageBusy=false;button.Content="\uE72C";button.IsEnabled=true;}
 }
 static void DescribeUsageRefresh(int result,List<UsageWindow> rows,DateTime attempt,DateTime now,out string glyph,out string description){
  var latest=rows==null?DateTime.MinValue:rows.Where(slot=>!slot.Expired(now)).Select(slot=>slot.Sample).DefaultIfEmpty().Max();
  string failure=result==1?"Claude Code CLIが見つかりません":result==3?"照会がタイムアウトしました":result!=0?"CLIの照会に失敗しました。ログイン状態を確認してください":
   rows==null?"使用記録を読み込めません":latest==DateTime.MinValue?"有効な使用記録がありません":null;
  if(failure!=null){glyph="\uE7BA";description="更新失敗："+failure;return;}
  if(latest<attempt){glyph="\uE823";description="保存済みの使用枠を表示（取得 "+FormatUsageTime(latest)+"）";return;}
  glyph="\uE73E";description="Claude Code 使用枠を取得しました";
 }
 static string FormatUsageTime(DateTime stamp){return stamp.ToLocalTime().ToString("yyyy/MM/dd HH:mm",CultureInfo.InvariantCulture);}
 static FrameworkElement UsageTile(UsageWindow slot,DateTime now,bool fallback){
  bool expired=slot.Expired(now);
  string percent=expired?"—":slot.Percent.ToString("0.#")+"%";
  var ring=new Grid{Width=52,Height=52,Margin=new Thickness(0,0,7,0)};
  Brush track=SystemParameters.HighContrast?SystemColors.GrayTextBrush:Color("#DEE7E9");
  Brush ink=SystemParameters.HighContrast?SystemColors.WindowTextBrush:Color(slot.Percent>=95?"#AD422E":slot.Percent>=80?"#A56910":"#157B83");
  var rim=new Ellipse{Stroke=track,StrokeThickness=4};
  if(expired)rim.StrokeDashArray=new DoubleCollection{1,1};
  ring.Children.Add(rim);
  if(!expired && slot.Percent>=100)rim.Stroke=ink;
  else if(!expired && slot.Percent>0){
   double angle=slot.Percent*Math.PI/50;
   var arc=new ArcSegment(new Point(26+24*Math.Sin(angle),26-24*Math.Cos(angle)),new Size(24,24),0,slot.Percent>50,SweepDirection.Clockwise,true);
   var figure=new PathFigure(new Point(26,2),new[]{arc},false);
   ring.Children.Add(new System.Windows.Shapes.Path{Data=new PathGeometry(new[]{figure}),Stroke=ink,StrokeThickness=4});
  }
  var value=Label(percent,14);value.HorizontalAlignment=HorizontalAlignment.Center;value.FontWeight=FontWeights.Bold;ring.Children.Add(value);
  var text=new StackPanel{VerticalAlignment=VerticalAlignment.Center,MaxWidth=224};
  var name=UsageText(ShortPeriod(slot.Name));name.ToolTip=slot.Name;name.FontWeight=FontWeights.SemiBold;text.Children.Add(name);
  string detail="リセット予定 "+(slot.Reset==DateTime.MinValue?"未設定":FormatUsageTime(slot.Reset));
  if(expired)detail="期限切れ · 前回値 "+slot.Percent.ToString("0.#")+"%\n"+detail;
  else if(fallback)detail="前回値 "+percent+"\n"+detail;
  text.Children.Add(UsageLine("\uE72C",slot.Reset==DateTime.MinValue?"—":FormatUsageTime(slot.Reset),"リセット予定 "+(slot.Reset==DateTime.MinValue?"未設定":FormatUsageTime(slot.Reset))));
  if(expired || fallback)text.Children.Add(UsageLine("\uE81C",expired?slot.Percent.ToString("0.#")+"%":percent,detail));
  else{
   string glyph,stamp;var forecast=slot.Forecast(now,out glyph,out stamp);
   text.Children.Add(UsageLine(glyph,stamp,forecast));detail+="\n"+forecast;
  }
  var row=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(0,2,8,3)};
  row.Children.Add(ring);row.Children.Add(text);
  row.ToolTip=slot.Name+"\n"+detail;
  string accessibleValue=!expired && !fallback?"実測 "+percent+" ":"";
  AutomationProperties.SetName(row,slot.Name+" "+accessibleValue+detail);
  return row;
 }
 static List<UsageWindow> LoadUsage(Func<JavaScriptSerializer,List<UsageWindow>> load){
  try{return load(new JavaScriptSerializer{MaxJsonLength=16*1024*1024});}
  catch(IOException){}catch(UnauthorizedAccessException){}catch(ArgumentException){}catch(InvalidOperationException){}
  return null;
 }
 static string ShortPeriod(string name){return name.Replace("時間枠","h").Replace("日枠","d").Replace("分枠","m");}
 static Brush UsageInk(){return SystemParameters.HighContrast?SystemColors.HighlightTextBrush:Brushes.White;}
 static Brush UsageFill(string glyph){
  if(SystemParameters.HighContrast)return SystemColors.HighlightBrush;
  switch(glyph){
   case "\uE72C":return Color("#16636C");
   case "\uE9D9":return Color("#50468E");
   case "\uE7BA":return Color("#99421E");
   case "\uE73E":return Color("#246844");
   default:return Color("#455D70");
  }
 }
 static Border UsageBadge(string glyph,string description){
  var mark=new TextBlock{Text=glyph,FontFamily=new FontFamily("Segoe MDL2 Assets"),FontSize=14,
   Foreground=UsageInk(),HorizontalAlignment=HorizontalAlignment.Center,VerticalAlignment=VerticalAlignment.Center};
  var badge=new Border{Width=22,Height=22,CornerRadius=new CornerRadius(11),Margin=new Thickness(0,1,5,1),
   Background=UsageFill(glyph),Child=mark,ToolTip=description};
  AutomationProperties.SetName(badge,description);
  return badge;
 }
 static void UsageNotice(Border host,string glyph,string description){
  host.Child=UsageBadge(glyph,description);
 }
 static FrameworkElement UsageLine(string glyph,string value,string description){
  var line=new StackPanel{Orientation=Orientation.Horizontal,ToolTip=description};
  AutomationProperties.SetName(line,description);
  line.Children.Add(UsageBadge(glyph,description));
  if(value.Length>0){var text=UsageText(value);text.VerticalAlignment=VerticalAlignment.Center;line.Children.Add(text);}
  return line;
 }
 static TextBlock UsageText(string text){return new TextBlock{Text=text,FontSize=13,TextWrapping=TextWrapping.Wrap};}
}
