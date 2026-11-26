using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Xml.Linq;
using System.Runtime.InteropServices;

internal sealed partial class Desktop {
 sealed class MapNode {
  public string Path,Kind;
  public readonly List<MapTask> Tasks=new List<MapTask>();
  public readonly List<MapLocation> Locations=new List<MapLocation>();
  public string Caption {get{return System.IO.Path.GetFileName(Path);}}
  public readonly Dictionary<MapNode,string> From=new Dictionary<MapNode,string>(), To=new Dictionary<MapNode,string>();
  public override string ToString(){return Path;}
  public void Connect(MapNode target,string relation){
   if(this==target)return;
   string previous;
   if(To.TryGetValue(target,out previous) && previous!=relation)relation=previous+"・"+relation;
   To[target]=relation;target.From[this]=relation;
  }
 }
 sealed class MapTask { public string Key,Title,State,ParentTitle; }
 sealed class MapLocation {
  public int Line;
  public string Title;
  public override string ToString(){return Line+" 行　"+Title;}
 }
 static void ReadMapLocations(MapNode node,string body,bool markdown){
  using(var reader=new StringReader(body)){
   string line;int number=0;string fence=null;
   while((line=reader.ReadLine())!=null){
    number++;string trimmed=line.TrimStart();
    if(markdown){
     if(fence!=null){if(trimmed.StartsWith(fence) && trimmed.TrimEnd(fence[0]).Trim().Length==0)fence=null;continue;}
     if(trimmed.StartsWith("```") || trimmed.StartsWith("~~~")){fence=new string(trimmed[0],trimmed.TakeWhile(c=>c==trimmed[0]).Count());continue;}
     int hashes=trimmed.TakeWhile(c=>c=='#').Count();
     if(hashes==0 || hashes>6 || hashes==trimmed.Length || !Char.IsWhiteSpace(trimmed[hashes]))continue;
     trimmed=trimmed.Substring(hashes).Trim().TrimEnd('#').TrimEnd();
    }else if(!ProjectMap.IsModuleLine(trimmed))continue;
    if(trimmed.Length>0)node.Locations.Add(new MapLocation{Line=number,Title=trimmed});
   }
  }
 }
 sealed class ProjectMap {
  public readonly Dictionary<string,MapNode> Nodes=new Dictionary<string,MapNode>(StringComparer.OrdinalIgnoreCase);
  public int Skipped;
  static readonly HashSet<string> Omit=new HashSet<string>(".git .vs .idea node_modules target bin obj dist build .venv venv __pycache__ Library Temp Logs".Split(' '),StringComparer.OrdinalIgnoreCase);
  static readonly HashSet<string> Source=new HashSet<string>(".rs .cs .xaml .ps1 .js .jsx .ts .tsx .mjs .cjs .vue .py .go .java .kt .swift .c .cpp .h .hpp .rb .fs .csproj .sln .asmdef".Split(' '),StringComparer.OrdinalIgnoreCase);
  static readonly HashSet<string> Manifest=new HashSet<string>("Cargo.toml package.json pyproject.toml go.mod CMakeLists.txt".Split(' '),StringComparer.OrdinalIgnoreCase);
  static readonly Regex Links=new Regex(@"(?<!!)\[[^\]\r\n]*\]\(\s*(?:<([^>\r\n]+)>|((?:[^()\s]|\([^()\r\n]*\))+))",RegexOptions.Compiled);
  static readonly Regex Modules=new Regex(@"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_]\w*)\s*;",RegexOptions.Compiled);
  public static bool IsModuleLine(string line){return Modules.IsMatch(line);}
  public void AddTasks(string root,XElement snapshot){
   var rows=snapshot.Elements("task").ToList();
   var titles=rows.ToDictionary(row=>Value(row,"key"),row=>Value(row,"title"));
   root=System.IO.Path.GetFullPath(root).TrimEnd('\\')+"\\";
   foreach(var row in rows){
    string parent;titles.TryGetValue(Value(row,"parent"),out parent);
    var task=new MapTask{Key=Value(row,"key"),Title=Value(row,"title"),State=states[(int)row.Attribute("state")],ParentTitle=parent};
    var targets=new HashSet<MapNode>{Nodes["."]};
    foreach(var doc in row.Elements("doc")){
     try{
      string full=System.IO.Path.GetFullPath(System.IO.Path.Combine(root,PathName(doc.Value)));
      if(!full.StartsWith(root,StringComparison.OrdinalIgnoreCase))continue;
      string relative=full.Substring(root.Length);
      while(!String.IsNullOrEmpty(relative)){
       MapNode node;if(Nodes.TryGetValue(relative,out node))targets.Add(node);
       relative=System.IO.Path.GetDirectoryName(relative);
      }
     }catch(ArgumentException){}catch(NotSupportedException){}catch(IOException){}
    }
    foreach(var node in targets)node.Tasks.Add(task);
   }
  }
  public static ProjectMap Read(string root,CancellationToken token,HashSet<string> otherProjects=null){
   root=System.IO.Path.GetFullPath(root).TrimEnd('\\');
   var map=new ProjectMap();
   var top=new MapNode{Path=".",Kind="プロジェクト"};map.Nodes.Add(".",top);
   var dirs=new Queue<MapNode>();dirs.Enqueue(top);
   while(dirs.Count>0){
    token.ThrowIfCancellationRequested();
    var parent=dirs.Dequeue();
    var directory=new DirectoryInfo(System.IO.Path.Combine(root,parent.Path));
    try{
     foreach(var entry in directory.EnumerateFileSystemInfos()){
      token.ThrowIfCancellationRequested();

      if((entry.Attributes&FileAttributes.ReparsePoint)!=0)continue;
      bool dir=entry is DirectoryInfo;
      if(dir && (Omit.Contains(entry.Name) || (otherProjects!=null && otherProjects.Contains(entry.FullName))))continue;
      string ext=entry.Extension.ToLowerInvariant();
      string kind=dir?"フォルダー":ext==".md" || ext==".markdown"?"Markdown":Manifest.Contains(entry.Name)?"モジュール定義":Source.Contains(ext)?"ソース":null;
      if(kind==null || (otherProjects!=null && !dir && kind!="Markdown"))continue;
      string path=entry.FullName.Substring(root.Length+1);
      var node=new MapNode{Path=path,Kind=kind};map.Nodes.Add(path,node);parent.Connect(node,"構成");
      if(dir)dirs.Enqueue(node);
     }
    }catch(UnauthorizedAccessException){map.Skipped++;}catch(IOException){map.Skipped++;}
   }
   foreach(var node in map.Nodes.Values){
    token.ThrowIfCancellationRequested();
    bool md=node.Kind=="Markdown";
    if(!md && !node.Path.EndsWith(".rs",StringComparison.OrdinalIgnoreCase))continue;
    string file=System.IO.Path.Combine(root,node.Path),folder=System.IO.Path.GetDirectoryName(file);
    try{
     if(new FileInfo(file).Length>2*1024*1024){map.Skipped++;continue;}
     string body=File.ReadAllText(file);
     ReadMapLocations(node,body,md);
     if(md){
      foreach(Match m in Links.Matches(body)){
       var link=(m.Groups[1].Success?m.Groups[1]:m.Groups[2]).Value.Split('#','?')[0];
       if(link.Length==0 || link.Contains(":") || link.StartsWith("/") || link.StartsWith("\\"))continue;
       try{
        string full=System.IO.Path.GetFullPath(System.IO.Path.Combine(folder,Uri.UnescapeDataString(link))).TrimEnd('\\');
        MapNode dest;
        if(full.StartsWith(root+"\\",StringComparison.OrdinalIgnoreCase) && map.Nodes.TryGetValue(full.Substring(root.Length+1),out dest))node.Connect(dest,"リンク");
       }catch(ArgumentException){}catch(NotSupportedException){}
      }
     }else{
      string name=System.IO.Path.GetFileNameWithoutExtension(file);
      if(name!="lib" && name!="main" && name!="mod")folder=System.IO.Path.Combine(folder,name);
      foreach(Match m in Modules.Matches(body)){
       string mod=System.IO.Path.Combine(folder,m.Groups[1].Value).Substring(root.Length+1);
       MapNode dest;
       if(map.Nodes.TryGetValue(mod+".rs",out dest) || map.Nodes.TryGetValue(mod+"\\mod.rs",out dest))node.Connect(dest,"mod 宣言");
      }
     }
    }catch(IOException){map.Skipped++;}catch(UnauthorizedAccessException){map.Skipped++;}
   }
   return map;
  }
 }
 static string MapColor(MapNode n){
  switch(n.Kind){case "プロジェクト":return "#433B38";case "Markdown":return "#17628A";case "フォルダー":return "#805C16";default:return "#3D6470";}
 }
 static string MapGlyph(MapNode n){
  switch(n.Kind){case "プロジェクト":return "\uE821";case "Markdown":return "\uE8A5";case "フォルダー":return "\uE8B7";default:return "\uE943";}
 }
 static void PlaceMap(UIElement element,Canvas canvas,double x,double y){
  Canvas.SetLeft(element,x);Canvas.SetTop(element,y);canvas.Children.Add(element);
 }
 static void MapArrow(Canvas canvas,double x,double y,double end,double targetY){
  Brush ink=SystemParameters.HighContrast?SystemColors.WindowTextBrush:Color("#758285");
  double mid=(x+end)/2;
  var curve=new PathFigure{StartPoint=new Point(x,y)};
  curve.Segments.Add(new BezierSegment(new Point(mid,y),new Point(mid,targetY),new Point(end-6,targetY),true));
  var geometry=new PathGeometry();geometry.Figures.Add(curve);
  canvas.Children.Add(new System.Windows.Shapes.Path{Data=geometry,Stroke=ink,StrokeThickness=1.6,IsHitTestVisible=false});
  canvas.Children.Add(new System.Windows.Shapes.Polygon{Fill=ink,IsHitTestVisible=false,Points=new PointCollection{new Point(end,targetY),new Point(end-8,targetY-4),new Point(end-8,targetY+4)}});
 }
 static FrameworkElement MapCard(MapNode n,string name,string relation,double width,bool selected,Action follow){
  Brush accent=SystemParameters.HighContrast?SystemColors.WindowTextBrush:Color(MapColor(n));
  var body=new StackPanel{Width=width,ToolTip=n.Kind+"　"+n.Path};
  var glyph=new TextBlock{Text=MapGlyph(n),FontFamily=explorerIcons,FontSize=18,Foreground=SystemColors.WindowBrush,HorizontalAlignment=HorizontalAlignment.Center,VerticalAlignment=VerticalAlignment.Center};
  var circle=new Border{Width=34,Height=34,CornerRadius=new CornerRadius(17),HorizontalAlignment=HorizontalAlignment.Center,Background=accent,Child=glyph};
  body.Children.Add(circle);
  body.Children.Add(new TextBlock{Text=name,TextAlignment=TextAlignment.Center,FontWeight=FontWeights.SemiBold,TextTrimming=TextTrimming.CharacterEllipsis});
  body.Children.Add(new TextBlock{Text=relation.Length==0?n.Kind:relation+" · "+n.Kind,TextAlignment=TextAlignment.Center,FontSize=11,Foreground=accent,TextTrimming=TextTrimming.CharacterEllipsis});
  var card=body;
  if(selected)return card;
  var button=new Button{Content=card,Background=Brushes.Transparent,BorderThickness=new Thickness(0),Padding=new Thickness(0),Margin=new Thickness(0),ToolTip=card.ToolTip};
  System.Windows.Automation.AutomationProperties.SetName(button,relation+"　"+name);button.Click+=(s,e)=>follow();
  return button;
 }
 static KeyValuePair<MapNode,string>[] MapNeighbors(Dictionary<MapNode,string> edges){
  // Interleave types so a project with many files still shows its files and folders.
  var groups=edges.OrderBy(v=>v.Key.Path,StringComparer.OrdinalIgnoreCase).GroupBy(v=>v.Key.Kind)
   .OrderBy(g=>g.Key=="Markdown"?0:g.Key=="フォルダー"?1:2)
   .SelectMany(g=>g.Select((v,i)=>new{v,i})).OrderBy(a=>a.i);
  return groups.Select(a=>a.v).ToArray();
 }
 static int DrawProjectMap(Canvas canvas,MapNode focus,string project,int page,double available,Action<MapNode> follow){
  canvas.Children.Clear();
  var left=MapNeighbors(focus.From);var right=MapNeighbors(focus.To);
  const int perPage=4;
  int pages=Math.Max(1,(Math.Max(left.Length,right.Length)+perPage-1)/perPage);
  var sides=new[]{left.Skip(page*perPage).Take(perPage).ToArray(),right.Skip(page*perPage).Take(perPage).ToArray()};
  double width=Math.Max(630,available),gap=Math.Max(32,Math.Min(78,width*.075)),w=(width-2*gap-24)/3;
  double x0=12,x1=x0+w+gap,x2=x1+w+gap;
  int rows=Math.Max(1,Math.Max(sides[0].Length,sides[1].Length));
  double center=55+(rows-1)*43+31;
  canvas.Width=width;canvas.Height=112+rows*86;
  for(int side=0;side<2;side++){
   var nodes=sides[side];double x=side==0?x0:x2;
   PlaceMap(Label(side==0?"親・参照元  ("+left.Length+")":"配下・参照先  ("+right.Length+")",14),canvas,x,12);
   for(int i=0;i<nodes.Length;i++){
    var edge=nodes[i];var n=edge.Key;double cy=center+(i-(nodes.Length-1)/2.0)*86;
    if(side==0)MapArrow(canvas,x0+w/2+17,cy-14,x1+w/2-17,center-14);else MapArrow(canvas,x1+w/2+17,center-14,x2+w/2-17,cy-14);
    PlaceMap(MapCard(n,n.Path=="."?project:n.Caption,edge.Value,w,false,()=>follow(n)),canvas,x,cy-31);
   }
  }
  PlaceMap(MapCard(focus,focus.Path=="."?project:focus.Caption,"",w,true,null),canvas,x1,center-31);
  return pages;
 }
 sealed class WholeMapView : FrameworkElement {
  const double CardWidth=174,CardHeight=86,Column=212,Row=102;
  readonly Dictionary<MapNode,Point> positions=new Dictionary<MapNode,Point>();
  readonly Dictionary<int,List<MapNode>> columns=new Dictionary<int,List<MapNode>>();
  DrawingGroup drawing;
  readonly MapNode root;
  readonly string project;
  readonly Dictionary<MapNode,List<MapNode>> children;
  readonly Dictionary<MapNode,MapNode> owner=new Dictionary<MapNode,MapNode>();
  bool showAll,compact,selectedClosed;
  readonly HashSet<MapNode> closedInFull=new HashSet<MapNode>();
  readonly HashSet<MapNode> openBranches=new HashSet<MapNode>();
  readonly Brush background=SystemColors.WindowBrush;
  MapNode selected;
  public Point CenterOf(MapNode n){var p=positions[n];return new Point(p.X+CardWidth/2,p.Y+CardHeight/2);}
  public void SelectNode(MapNode n){
   if(selected==n)return;
   selected=n;selectedClosed=false;
   if(showAll){
    var seen=new HashSet<MapNode>();
    for(var parent=n;parent!=null && seen.Add(parent);){closedInFull.Remove(parent);MapNode next;parent=owner.TryGetValue(parent,out next)?next:null;}
   }
   ArrangeNodes();
  }
  public void ShowAll(bool value){showAll=value;closedInFull.Clear();selectedClosed=false;ArrangeNodes();}
  public void ActivateNode(MapNode n){
   bool wasOpen=openBranches.Contains(n);
   selected=n;selectedClosed=wasOpen;
   if(showAll){if(wasOpen)closedInFull.Add(n);else closedInFull.Remove(n);}
   ArrangeNodes();
  }
  public void Align(bool byLevel){compact=byLevel;ArrangeNodes();}
  public WholeMapView(ProjectMap map,string project,Action<MapNode> select){
   this.project=project;root=map.Nodes["."];
   // Lay out ownership first. Document links and module references may point back up the tree.
   children=map.Nodes.Values.ToDictionary(n=>n,n=>new List<MapNode>());
   foreach(var n in map.Nodes.Values.Where(n=>n!=root)){
    var parent=n.From.FirstOrDefault(e=>e.Value=="構成").Key;
    owner[n]=parent??root;children[owner[n]].Add(n);
   }
   foreach(var list in children.Values)list.Sort((a,b)=>StringComparer.OrdinalIgnoreCase.Compare(a.Path,b.Path));
   MouseLeftButtonUp+=(s,e)=>{var n=HitNode(e.GetPosition(this));if(n!=null){ActivateNode(n);select(n);}};
   MouseMove+=(s,e)=>{var n=HitNode(e.GetPosition(this));ToolTip=n==null?null:n.Kind+"　"+(n==root?project:n.Path);};
   selected=root;ArrangeNodes();
  }
  void ArrangeNodes(){
   var expanded=new HashSet<MapNode>{root};
   for(var n=selected;n!=null && n!=root && expanded.Add(n);){
    MapNode parent;n=owner.TryGetValue(n,out parent)?parent:null;
   }
   if(selectedClosed)expanded.Remove(selected);
   positions.Clear();columns.Clear();openBranches.Clear();
   drawing=new DrawingGroup();
   var visited=new HashSet<MapNode>();
   var stack=new Stack<Tuple<MapNode,int,bool>>();
   double leaf=12;
   foreach(var start in new[]{root}){
    if(visited.Contains(start))continue;
    stack.Push(Tuple.Create(start,0,false));
    while(stack.Count>0){
     var frame=stack.Pop();var n=frame.Item1;int level=frame.Item2;
     if(!frame.Item3){
      if(!visited.Add(n))continue;
      stack.Push(Tuple.Create(n,level,true));
      if(children[n].Count>0 && (showAll?!closedInFull.Contains(n):expanded.Contains(n))){
       openBranches.Add(n);
       foreach(var child in Enumerable.Reverse(children[n]))stack.Push(Tuple.Create(child,level+1,false));
      }
      continue;
     }
     var laidOut=children[n].Where(positions.ContainsKey).ToArray();
     double y=leaf;
     if(laidOut.Length==0)leaf+=Row;
     else y=(positions[laidOut[0]].Y+positions[laidOut[laidOut.Length-1]].Y)/2;
     positions[n]=new Point(14+Column*level,y);
     List<MapNode> group;if(!columns.TryGetValue(level,out group))columns[level]=group=new List<MapNode>();group.Add(n);
    }
   }
   if(compact){
    foreach(var group in columns.Values){
     double y=12;
     foreach(var n in group){positions[n]=new Point(positions[n].X,y);y+=Row;}
    }
   }
   Width=positions.Values.Max(p=>p.X)+CardWidth+14;Height=positions.Values.Max(p=>p.Y)+CardHeight+14;
   var structural=new Pen(SystemParameters.HighContrast?SystemColors.WindowTextBrush:Color("#819491"),1.4);
   var reference=new Pen(SystemParameters.HighContrast?SystemColors.WindowTextBrush:Color("#5778A1"),1.1){DashStyle=DashStyles.Dash};
   var font=new Typeface("Yu Gothic UI");
   using(var dc=drawing.Open()){
    foreach(var origin in positions){
     foreach(var edge in origin.Key.To){
      Point b;if(!positions.TryGetValue(edge.Key,out b))continue;
      Point a=origin.Value;
      bool forward=b.X>a.X;
      Point from=new Point(a.X+CardWidth/2+(forward?22:-22),a.Y+24);
      Point to=new Point(b.X+CardWidth/2+(forward?-22:22),b.Y+24);
      MapNode parent;bool tree=owner.TryGetValue(edge.Key,out parent) && parent==origin.Key;
      var pen=tree?structural:reference;
      var curve=new StreamGeometry();
      using(var ctx=curve.Open()){
       ctx.BeginFigure(from,false,false);
       double bend=forward?32:-32;
       if(a.X==b.X){from=new Point(a.X+CardWidth/2+22,from.Y);ctx.BeginFigure(from,false,false);bend=40;}
       ctx.BezierTo(new Point(from.X+bend,from.Y),new Point(to.X+(a.X==b.X?bend:-bend),to.Y),to,true,false);
      }
      curve.Freeze();dc.DrawGeometry(null,pen,curve);
      double tip=forward?-6:6;
      dc.DrawLine(pen,new Point(to.X+tip,to.Y-3),to);dc.DrawLine(pen,new Point(to.X+tip,to.Y+3),to);
     }
    }
    foreach(var item in positions){
     var n=item.Key;var p=item.Value;
     Brush color=SystemParameters.HighContrast?SystemColors.WindowTextBrush:Color(MapColor(n));
     dc.DrawEllipse(color,null,new Point(p.X+CardWidth/2,p.Y+24),22,22);
     var glyph=new FormattedText(MapGlyph(n),System.Globalization.CultureInfo.CurrentCulture,FlowDirection.LeftToRight,new Typeface(explorerIcons,FontStyles.Normal,FontWeights.Normal,FontStretches.Normal),21,background);
     dc.DrawText(glyph,new Point(p.X+(CardWidth-glyph.Width)/2,p.Y+24-glyph.Height/2));
     if(children[n].Count>0){
      var center=new Point(p.X+CardWidth/2+18,p.Y+40);
      dc.DrawEllipse(background,new Pen(color,1),center,7,7);
      var stroke=new Pen(color,1.5);
      dc.DrawLine(stroke,new Point(center.X-3.5,center.Y),new Point(center.X+3.5,center.Y));
      if(!openBranches.Contains(n))dc.DrawLine(stroke,new Point(center.X,center.Y-3.5),new Point(center.X,center.Y+3.5));
     }
     string name=n==root?project:n.Caption;
     foreach(bool caption in new[]{false,true}){
      var text=new FormattedText(caption?name:n.Kind,System.Globalization.CultureInfo.CurrentCulture,FlowDirection.LeftToRight,font,caption?14:11,caption?SystemColors.WindowTextBrush:color);
      text.MaxTextWidth=CardWidth;text.TextAlignment=TextAlignment.Center;text.MaxLineCount=1;text.Trimming=TextTrimming.CharacterEllipsis;
      dc.DrawText(text,new Point(p.X,p.Y+(caption?48:68)));
     }
    }
   }
   drawing.Freeze();
   InvalidateVisual();
  }
  MapNode HitNode(Point point){
   int column=(int)Math.Floor((point.X-14)/Column);List<MapNode> nodes;
   if(!columns.TryGetValue(column,out nodes))return null;
   return nodes.FirstOrDefault(n=>new Rect(positions[n],new Size(CardWidth,CardHeight)).Contains(point));
  }
  protected override void OnRender(DrawingContext dc){
   base.OnRender(dc);dc.DrawRectangle(background,null,new Rect(0,0,Width,Height));dc.DrawDrawing(drawing);
   if(selected!=null){var p=positions[selected];dc.DrawEllipse(null,new Pen(SystemColors.HighlightBrush,2),new Point(p.X+CardWidth/2,p.Y+24),25,25);}
  }
 }
 void ShowMapSourceLine(string file,int line){
  explorerPath=file;At<TextBlock>("FileName").Text=file+"　"+line+" 行";At<Button>("Attach").Visibility=Visibility.Collapsed;
  var view=At<ScrollViewer>("ExplorerPreview");view.Visibility=Visibility.Visible;
  try{
   var excerpt=File.ReadLines(file).Skip(Math.Max(0,line-5)).Take(60).ToArray();
   var lines=new StackPanel();int number=Math.Max(1,line-4);
   foreach(string value in excerpt){
    var text=PreviewLabel(number+"　"+value);
    if(number==line){text.FontWeight=FontWeights.Bold;text.Background=SystemColors.HighlightBrush;text.Foreground=SystemColors.HighlightTextBrush;}
    lines.Children.Add(text);number++;
   }
   view.Content=lines;
  }catch(IOException error){view.Content=PreviewLabel(error.Message);}catch(UnauthorizedAccessException error){view.Content=PreviewLabel(error.Message);}
  view.ScrollToTop();
 }
 void ShowProjectMap(string path,int index){
  var dialog=new Window{Title=ProjectName(path)+" の関連図",Owner=window,Width=1160,Height=680,MinWidth=680,MinHeight=390,WindowStartupLocation=WindowStartupLocation.CenterOwner};
  var panel=new DockPanel{Margin=new Thickness(16)};
  var picker=new ComboBox{Margin=new Thickness(0,0,0,12),MaxDropDownHeight=360};
  DockPanel.SetDock(picker,Dock.Top);panel.Children.Add(picker);
  var status=Label("プロジェクトを読み込み中…",13);DockPanel.SetDock(status,Dock.Bottom);panel.Children.Add(status);
  var toolbar=new DockPanel{Margin=new Thickness(0,0,0,8)};
  var back=new Button{Content="← 戻る",IsEnabled=false};
  var open=new Button{Visibility=Visibility.Collapsed};
  toolbar.Children.Add(back);
  toolbar.Children.Add(new TextBlock{Text=ProjectName(path),FontWeight=FontWeights.SemiBold,VerticalAlignment=VerticalAlignment.Center,Margin=new Thickness(10,0,0,0)});
  DockPanel.SetDock(toolbar,Dock.Top);panel.Children.Insert(0,toolbar);
  var navigation=new StackPanel{Orientation=Orientation.Horizontal,HorizontalAlignment=HorizontalAlignment.Center};
  var prev=new Button{Content="‹ 前へ"};var next=new Button{Content="次へ ›"};var counter=Label("",14);
  navigation.Children.Add(prev);navigation.Children.Add(counter);navigation.Children.Add(next);
  DockPanel.SetDock(navigation,Dock.Bottom);panel.Children.Add(navigation);
  var canvas=new Canvas{Background=SystemParameters.HighContrast?SystemColors.WindowBrush:Color("#F5F3F0")};
  var scroll=new ScrollViewer{Content=canvas,VerticalScrollBarVisibility=ScrollBarVisibility.Auto,HorizontalScrollBarVisibility=ScrollBarVisibility.Auto};
  var detail=new StackPanel{Margin=new Thickness(14,8,10,8)};
  var category=Label("",13);var title=Label("",18);title.FontWeight=FontWeights.SemiBold;title.TextWrapping=TextWrapping.Wrap;
  var location=Label("",13);location.TextWrapping=TextWrapping.Wrap;
  var sections=new ComboBox{Margin=new Thickness(0,10,0,4),ToolTip="移動先を選択"};
  var related=new ComboBox{Margin=new Thickness(0,8,0,4),ToolTip="関連する項目を選択"};
  var taskHeading=Label("",15);taskHeading.FontWeight=FontWeights.SemiBold;taskHeading.Margin=new Thickness(0,12,0,4);
  var taskRows=new StackPanel();
  var relatedLabel=Label("関連項目へ移動",13);
  foreach(var control in new UIElement[]{category,title,location,sections,open,taskHeading,taskRows,relatedLabel,related})detail.Children.Add(control);
  var inspector=new Border{Background=SystemColors.WindowBrush,BorderBrush=SystemColors.ControlDarkBrush,BorderThickness=new Thickness(1,0,0,0),Child=new ScrollViewer{Content=detail,VerticalScrollBarVisibility=ScrollBarVisibility.Auto}};
  var split=new Grid();
  split.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(3,GridUnitType.Star)});
  split.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(1,GridUnitType.Star),MinWidth=190,MaxWidth=330});
  Grid.SetColumn(inspector,1);split.Children.Add(scroll);split.Children.Add(inspector);panel.Children.Add(split);dialog.Content=panel;
  related.SelectionChanged+=(s,e)=>{var choice=related.SelectedItem as ComboBoxItem;if(choice!=null)picker.SelectedItem=choice.Tag;};
  WholeMapView whole=null;bool overview=true,autoFit=false;double zoom=1;
  var tools=new StackPanel{Orientation=Orientation.Horizontal};
  var mode=new Button{Content="選択項目の周辺"};
  var fit=new Button{Content="表示範囲に合わせる"};var minus=new Button{Content="−",ToolTip="縮小"};var plus=new Button{Content="＋",ToolTip="拡大"};
  var actual=new Button{Content="100%",ToolTip="項目名を原寸で読む"};
  var all=new Button{Content="全階層を表示"};bool allLevels=false;
  var align=new Button{Content="整列 ▾"};
  var order=new ContextMenu();
  foreach(bool byLevel in new[]{false,true}){
   var item=new MenuItem{Header=byLevel?"階層ごとに詰めて整列":"枝ごとに整列"};
   bool layout=byLevel;item.Click+=(s,e)=>{if(whole!=null)whole.Align(layout);};order.Items.Add(item);
  }
  align.Click+=(s,e)=>{order.PlacementTarget=align;order.Placement=System.Windows.Controls.Primitives.PlacementMode.Bottom;order.IsOpen=true;};
  foreach(var b in new[]{mode,all,align,fit,minus,plus,actual})tools.Children.Add(b);
  DockPanel.SetDock(tools,Dock.Right);toolbar.Children.Insert(0,tools);
  Action scale=()=>{
   if(whole==null || !overview)return;
   if(autoFit)zoom=Math.Min(1,Math.Min(Math.Max(1,scroll.ActualWidth-20)/whole.Width,Math.Max(1,scroll.ActualHeight-20)/whole.Height));
   whole.LayoutTransform=new ScaleTransform(zoom,zoom);
  };
  Action centerSelected=()=>{
   var n=picker.SelectedItem as MapNode;if(whole==null || n==null || autoFit || !overview)return;
   Point p=whole.CenterOf(n);scroll.UpdateLayout();scroll.ScrollToHorizontalOffset(p.X*zoom-scroll.ViewportWidth/2);scroll.ScrollToVerticalOffset(p.Y*zoom-scroll.ViewportHeight/2);
  };
  Action<double> setZoom=value=>{autoFit=false;zoom=Math.Min(4,value);scale();centerSelected();};
  all.Click+=(s,e)=>{
   if(whole==null)return;allLevels=!allLevels;whole.ShowAll(allLevels);
   all.Content=allLevels?"選択した枝のみ":"全階層を表示";autoFit=allLevels;zoom=1;scale();centerSelected();
  };
  order.Closed+=(s,e)=>{scale();centerSelected();};
  plus.Click+=(s,e)=>setZoom(zoom*1.5);minus.Click+=(s,e)=>setZoom(Math.Max(0.000001,zoom/1.5));actual.Click+=(s,e)=>setZoom(1);
  fit.Click+=(s,e)=>{autoFit=true;scale();};
  scroll.SizeChanged+=(s,e)=>{if(autoFit)scale();};
  var cancel=new CancellationTokenSource();dialog.Closed+=(s,e)=>cancel.Cancel();
  var history=new Stack<MapNode>();bool returning=false;int page=0;
  Action redraw=()=>{
   var n=picker.SelectedItem as MapNode;if(n==null)return;
   if(overview){navigation.Visibility=Visibility.Collapsed;if(whole!=null){whole.SelectNode(n);if(autoFit)scale();}return;}
   int count=DrawProjectMap(canvas,n,ProjectName(path),page,scroll.ViewportWidth,dest=>picker.SelectedItem=dest);
   prev.IsEnabled=page>0;next.IsEnabled=page+1<count;counter.Text="　"+(page+1)+" / "+count+"　";navigation.Visibility=count>1?Visibility.Visible:Visibility.Collapsed;
  };
  mode.Click+=(s,e)=>{
   if(whole==null)return;overview=!overview;mode.Content=overview?"選択項目の周辺":"プロジェクト全体";
   scroll.Content=overview?(FrameworkElement)whole:canvas;
   foreach(var b in new[]{all,align,fit,minus,plus,actual})b.Visibility=overview?Visibility.Visible:Visibility.Collapsed;
   page=0;scale();redraw();centerSelected();
  };
  prev.Click+=(s,e)=>{page--;redraw();};next.Click+=(s,e)=>{page++;redraw();};
  scroll.ScrollChanged+=(s,e)=>{if(e.ViewportWidthChange!=0)redraw();};
  picker.SelectionChanged+=(s,e)=>{
   var n=picker.SelectedItem as MapNode;if(n==null)return;
   if(!returning && e.RemovedItems.Count>0)history.Push((MapNode)e.RemovedItems[0]);
   back.IsEnabled=history.Count>0;page=0;if(!overview)scroll.ScrollToTop();redraw();centerSelected();
   category.Text=n.Kind;title.Text=n.Path=="."?ProjectName(path):n.Caption;
   location.Text=n.Path=="."?path:n.Path;
   taskHeading.Text="作業タスク（"+n.Tasks.Count+"件）";taskRows.Children.Clear();
   foreach(var task in n.Tasks){
    var caption=new StackPanel();
    caption.Children.Add(new TextBlock{Text=task.State+(String.IsNullOrEmpty(task.ParentTitle)?"":"　親："+task.ParentTitle),FontSize=12,TextWrapping=TextWrapping.Wrap});
    caption.Children.Add(new TextBlock{Text=task.Title,FontWeight=FontWeights.SemiBold,TextWrapping=TextWrapping.Wrap});
    var row=new Button{Content=caption,HorizontalContentAlignment=HorizontalAlignment.Left,ToolTip="タスク詳細を開く",Margin=new Thickness(0,0,0,4)};
    row.Click+=(sender,args)=>{dialog.Close();if(Run(746,0,task.Key))Run(160);};taskRows.Children.Add(row);
   }
   sections.Items.Clear();sections.Items.Add("先頭から");
   foreach(var mark in n.Locations)sections.Items.Add(mark);
   sections.SelectedIndex=0;sections.Visibility=Visible(n.Locations.Count>0);
   related.Items.Clear();
   foreach(var direction in new[]{n.From,n.To})foreach(var edge in direction.OrderBy(v=>v.Key.Path,StringComparer.OrdinalIgnoreCase))
    related.Items.Add(new ComboBoxItem{Content=(direction==n.From?"← ":"→ ")+edge.Value+"　"+(edge.Key.Path=="."?ProjectName(path):edge.Key.Caption),Tag=edge.Key,ToolTip=edge.Key.Path});
   relatedLabel.Visibility=related.Visibility=Visible(related.Items.Count>0);
   open.Content=n.Kind=="プロジェクト" || n.Kind=="フォルダー"?"フォルダーの内容を表示":"本文を開く";
   open.Visibility=Visibility.Visible;
  };
  back.Click+=(s,e)=>{if(history.Count==0)return;returning=true;try{picker.SelectedItem=history.Pop();}finally{returning=false;}};
  open.Click+=(s,e)=>{var n=picker.SelectedItem as MapNode;if(n==null)return;dialog.Close();
   if((flags&8)!=0 && !Run(197))return;
   string full=Path.Combine(path,n.Path);
   var mark=sections.SelectedItem as MapLocation;
   if(n.Kind=="Markdown"){
    explorerPath=full;if(!Run(717,index,n.Path) || mark==null)return;
    var list=At<ListBox>("DocumentHeadings");int pos=n.Locations.IndexOf(mark);
    if(pos>=0 && pos<list.Items.Count){
     var item=(ListBoxItem)list.Items[pos];var heading=item.Content as TextBlock;
     if(heading!=null && heading.Text==mark.Title){list.SelectedItem=item;list.ScrollIntoView(item);return;}
    }
   }
   FileSystemInfo entry=n.Kind=="フォルダー" || n.Kind=="プロジェクト"?(FileSystemInfo)new DirectoryInfo(full):new FileInfo(full);
   if(mark==null){PreviewExplorerEntry(entry);return;}
   ShowMapSourceLine(full,mark.Line);
  };
  dialog.Loaded+=async(s,e)=>{
   try{
    var snapshot=XElement.Parse(Marshal.PtrToStringUni(kagari_action(core,745,index,"")));
    if(snapshot.Name=="error")throw new InvalidOperationException(snapshot.Value);
    var map=await Task.Run(()=>{var result=ProjectMap.Read(path,cancel.Token);result.AddTasks(path,snapshot);return result;});
    if(cancel.IsCancellationRequested)return;
    whole=new WholeMapView(map,ProjectName(path),dest=>{
     if(picker.SelectedItem!=dest)picker.SelectedItem=dest;
     else{if(autoFit)scale();centerSelected();}
    });
    scroll.Content=whole;scale();
    picker.ItemsSource=map.Nodes.Values.OrderBy(n=>n.Path=="."?0:1).ThenBy(n=>n.Path,StringComparer.OrdinalIgnoreCase).ToArray();
    picker.SelectedItem=map.Nodes["."];
    status.Text="実線：構成・親子　　破線：資料・モジュール参照　・　"+map.Nodes.Count+" 項目"+(map.Skipped>0?"　（一部読み込み省略）":"");
   }catch(OperationCanceledException){}catch(Exception ex){if(!cancel.IsCancellationRequested)status.Text="読み込みに失敗しました　"+ex.Message;}

  };
  dialog.ShowDialog();
 }
}
