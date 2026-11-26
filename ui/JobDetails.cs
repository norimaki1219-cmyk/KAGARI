using System;
using System.Linq;
using System.Collections.Generic;
using System.Text.RegularExpressions;
using System.Web.Script.Serialization;

internal sealed partial class Desktop {
 public sealed partial class NewsItem {
  internal void NotifyJobByline(){var handler=PropertyChanged;if(handler!=null)handler(this,new System.ComponentModel.PropertyChangedEventArgs("JobByline"));}
  [ScriptIgnore] public string JobTime {get{return jobCollectedAt??collected;}}
  public string jobBody {get;set;}
  [ScriptIgnore] public string DisplayJobBody {get{return String.IsNullOrWhiteSpace(jobBody)?summary:jobBody;}}
  [ScriptIgnore] public string JobSkills {get{
   var matches=Regex.Matches((title??"")+" "+DisplayJobBody,@"\b(?:Python|FastAPI|Next\.js|Stable Diffusion|ChatGPT|Claude|Gemini|LLM|RAG|Docker|TypeScript|OpenAI API)\b",RegexOptions.IgnoreCase|RegexOptions.ECMAScript);
   return String.Join(" ／ ",matches.Cast<Match>().Select(m=>m.Value).Distinct(StringComparer.OrdinalIgnoreCase));
  }}
  public string jobCollectedAt {get;set;}
  [ScriptIgnore] public bool JobReviewed {get;set;}
  static string JobMatch(string text,string pattern){
   var match=Regex.Match(text??"",pattern,RegexOptions.IgnoreCase);
   return match.Success?match.Value.Trim():"未取得";
  }
  [ScriptIgnore] public string JobPay {get{
   string terms=significance??"";
   var part=terms.Split(new[]{" ／ ","／","\n"},StringSplitOptions.None)[0].Trim();
   if(part.StartsWith("予算 ") || part.StartsWith("月額") || part.StartsWith("時給") || part.StartsWith("固定") || part.StartsWith("見積"))return part.Replace("予算 ","");
   return JobMatch(terms,@"(?:月額|時給|日給|固定報酬|年収|報酬)[：:\s]*(?:上限|下限|約)?\s*[\d,.千万円～〜\- ]+円");
  }}
  [ScriptIgnore] public string JobPayType {get{
   string pay=JobPay;
   if(pay.Contains("月額") || pay.Contains("/月"))return "月額";
   if(pay.Contains("時給") || pay.Contains("/時"))return "時給";
   if(pay.Contains("見積"))return "見積り";
   if(pay.Contains("固定") || ((significance??"").StartsWith("予算 ") && pay.Contains("円")))return "固定";
   return "未取得";
  }}
  [ScriptIgnore] public string JobDeadline {get{return JobMatch(significance,@"(?<=締切)\s*\d{4}年\d{1,2}月\d{1,2}日|(?<=期限)\s*\d{4}[-/]\d{1,2}[-/]\d{1,2}");}}
  [ScriptIgnore] public string JobShortDeadline {get{return JobDeadline.Replace("年","/").Replace("月","/").Replace("日","");}}
  [ScriptIgnore] public string JobWork {get{return JobMatch((title??"")+" "+significance,@"フルリモート|完全在宅|リモートメイン|一部リモート|ハイブリッド|常駐|リモート可");}}
  [ScriptIgnore] public string JobDays {get{return JobMatch((title??"")+" "+significance,@"週\s*[0-9０-９]+(?:\s*[-～〜]\s*[0-9０-９]+)?\s*日");}}
  [ScriptIgnore] public string JobByline {get{return (JobReviewed?"":"未確認  ·  ")+JobSite+(JobDays=="未取得"?"":"  ·  "+JobDays);}}
  [ScriptIgnore] public string JobFacts {get{return "期限　"+JobDeadline+"\n稼働　"+JobDays+" ／ "+JobWork+"\n収集　"+(jobCollectedAt??collected??"未取得");}}
 }
 public sealed class JobPatrolInfo {
  public string publicPhase,dotsPhase,updated;
  public int targets,checkedCount,items;
  public string[] details;
 }
 static string JobKey(string url){
  Uri u;if(!Uri.TryCreate(url,UriKind.Absolute,out u))return url;
  if((u.Host=="coconala.com" && Regex.IsMatch(u.AbsolutePath,@"^/requests/\d+/?$")) ||
     (u.Host=="freelance.findy-code.io" && Regex.IsMatch(u.AbsolutePath,@"^/works/[\w-]+/?$")))
   return u.GetLeftPart(UriPartial.Path).TrimEnd('/');
  return new UriBuilder(u){Fragment=""}.Uri.AbsoluteUri;
 }
 static void NormalizeJobs(NewsState state){
  state.articles=state.articles.GroupBy(j=>JobKey(j.url)).Select(g=>g.First()).ToList();
  if(state.pinnedJobs!=null)state.pinnedJobs=state.pinnedJobs.Values.GroupBy(j=>JobKey(j.url)).ToDictionary(g=>g.Key,g=>g.First());
  if(state.jobFirstSeen!=null)state.jobFirstSeen=state.jobFirstSeen.GroupBy(p=>JobKey(p.Key)).ToDictionary(g=>g.Key,g=>g.Min(p=>p.Value));
 }
 static string JobsTimestamp(){return DateTimeOffset.UtcNow.ToOffset(TimeSpan.FromHours(9)).ToString("yyyy-MM-dd HH:mm");}
 static void PatrolChange(Action<JobPatrolInfo> change){
  UpdateFeed(jobsFile,s=>{if(s.jobPatrol==null)s.jobPatrol=new JobPatrolInfo();change(s.jobPatrol);s.jobPatrol.updated=JobsTimestamp();});
 }
}
