using System;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;

internal sealed partial class Desktop {
 static void TraceArrow(Canvas c,double x,double y,double endX,double endY,bool dashed,string color){
  var brush=Color(color);
  var edge=new Line{X1=x,Y1=y,X2=endX,Y2=endY,Stroke=brush,StrokeThickness=1.5};
  if(dashed)edge.StrokeDashArray=new DoubleCollection{3,3};
  c.Children.Add(edge);
  var direction=new Vector(endX-x,endY-y);direction.Normalize();
  var tip=new Point(endX,endY);var back=tip-direction*8;var side=new Vector(-direction.Y,direction.X)*4;
  c.Children.Add(new Polygon{Fill=brush,Points=new PointCollection{tip,back+side,back-side}});
 }
 static Button TraceNode(Canvas c,double x,double y,string title,string sub,string tip,string stroke,bool dashed,bool detected){
  var box=new Grid{Width=156,Height=52,ToolTip=tip};
  var frame=new Rectangle{RadiusX=6,RadiusY=6,Fill=Color("#192332"),Stroke=Color(stroke),StrokeThickness=detected?3:1.5};
  if(dashed)frame.StrokeDashArray=new DoubleCollection{3,2};
  box.Children.Add(frame);
  var labels=new StackPanel{VerticalAlignment=VerticalAlignment.Center,Margin=new Thickness(6,0,6,0)};
  foreach(var line in new[]{title,sub}){
   var label=Label(line,12);label.TextAlignment=TextAlignment.Center;label.Foreground=Brushes.White;labels.Children.Add(label);
  }
  box.Children.Add(labels);var button=new Button{Content=box,Padding=new Thickness(0),Margin=new Thickness(0),ToolTip=tip};System.Windows.Automation.AutomationProperties.SetName(button,title+" "+sub);Canvas.SetLeft(button,x);Canvas.SetTop(button,y);c.Children.Add(button);return button;
 }
 static FrameworkElement TraceGraph(object row,object path,object[] nodes,object[] events){
  var grid=new DockPanel{Background=Color("#101722"),Margin=new Thickness(0,5,0,8)};
  var bar=new DockPanel{Margin=new Thickness(6)};DockPanel.SetDock(bar,Dock.Top);grid.Children.Add(bar);
  var buttons=new StackPanel{Orientation=Orientation.Horizontal};DockPanel.SetDock(buttons,Dock.Right);bar.Children.Add(buttons);
  var hint=UsageText("ドラッグで移動 · Ctrl＋ホイールで拡大縮小");hint.Foreground=Brushes.White;bar.Children.Add(hint);
  var pattern=new DrawingGroup();
  pattern.Children.Add(new GeometryDrawing(Color("#101722"),null,new RectangleGeometry(new Rect(0,0,18,18))));
  pattern.Children.Add(new GeometryDrawing(Color("#28364A"),null,new EllipseGeometry(new Point(9,9),0.85,0.85)));
  var dots=new DrawingBrush(pattern){TileMode=TileMode.Tile,ViewportUnits=BrushMappingMode.Absolute,Viewport=new Rect(0,0,18,18)};dots.Freeze();
  var selected=UsageText("ノードを選択して詳細を表示");selected.Foreground=Brushes.White;selected.Margin=new Thickness(8);DockPanel.SetDock(selected,Dock.Bottom);grid.Children.Add(selected);
  var view=new Canvas{ClipToBounds=true,Background=dots,Focusable=true};grid.Children.Add(view);
  var graph=new Canvas();view.Children.Add(graph);
  double width=0,height=110;bool gap=!Equals(Field(nodes[0],"observed"),true)||Equals(Field(path,"truncated"),true);
  if(gap)TraceNode(graph,12,28,"記録の外","","これ以前の起動経路は記録されていません","#74869D",true,false).Click+=(s,e)=>selected.Text="開始元　未取得";
  for(int i=0;i<nodes.Length;i++){
   double x=12+(i+(gap?1:0))*200;var node=nodes[i];
   if(x>12)TraceArrow(graph,x-44,54,x,54,i==0,"#9BAFCC");
   bool observed=Equals(Field(node,"observed"),true);
   string time=observed?"起動 "+AlertTime(node,"start"):"起動は記録範囲外";
   string severity=AlertValue(row,"severity");
   string highlight=severity=="critical"||severity=="high"?"#EE8585":severity=="medium"?"#FFD267":"#83C6EA";
   TraceNode(graph,x,28,ProcessName(node),time.Replace(DateTime.Now.Year+"/",""),AlertValue(node,"image")+"\n"+time,i==nodes.Length-1?highlight:"#758CA6",!observed,i==nodes.Length-1).Click+=(s,e)=>selected.Text=GuardValue(node,"image")+"　"+time+"　　実行ID　"+GuardValue(node,"guid");
   width=x+168;
   var actions=events.Where(e=>AlertValue(e,"node")==i.ToString()).GroupBy(e=>new{Kind=TraceKind(e),Target=AlertValue(e,"target")});
   int j=0;
   foreach(var group in actions){
    double y=108+j++*67;
    graph.Children.Add(new Line{X1=x+6,Y1=j==1?80:y-41,X2=x+6,Y2=y+26,Stroke=Color("#AA8BAE")});
    TraceArrow(graph,x+6,y+26,x+24,y+26,false,"#D894BE");
    var e=group.First();string kind=group.Key.Kind+(group.Count()>1?" ×"+group.Count():"");
    TraceNode(graph,x+24,y,kind,group.Key.Target,kind+"\n"+group.Key.Target+"\n"+AlertTime(e,"ts"),"#CC8EB8",false,false).Click+=(sender,args)=>selected.Text=kind+"　"+GuardValue(e,"target")+"　"+AlertTime(e,"ts");
    height=Math.Max(height,y+64);width=Math.Max(width,x+192);
   }
  }
  var matrix=new MatrixTransform();graph.RenderTransform=matrix;
  Action fit=()=>{double z=Math.Min(1,Math.Min(view.ActualWidth/width,view.ActualHeight/height));if(z>0)matrix.Matrix=new Matrix(z,0,0,z,(view.ActualWidth-width*z)/2,(view.ActualHeight-height*z)/2);};
  Action<double,Point> zoom=(factor,p)=>{var m=matrix.Matrix;double z=Math.Min(5,Math.Max(0.025,m.M11*factor));m.ScaleAt(z/m.M11,z/m.M11,p.X,p.Y);matrix.Matrix=m;};
  foreach(string action in new[]{"＋","－","全体"}){
   var b=new Button{Content=action,MinWidth=32,Padding=new Thickness(5,2,5,2)};
   b.Click+=(s,e)=>{if(action=="全体")fit();else zoom(action=="＋"?1.25:0.8,new Point(view.ActualWidth/2,view.ActualHeight/2));};buttons.Children.Add(b);
  }
  view.Loaded+=(s,e)=>fit();
  Point last=new Point();
  view.MouseLeftButtonDown+=(s,e)=>{last=e.GetPosition(view);view.CaptureMouse();view.Cursor=Cursors.ScrollAll;e.Handled=true;};
  view.MouseMove+=(s,e)=>{if(!view.IsMouseCaptured)return;var now=e.GetPosition(view);var m=matrix.Matrix;m.Translate(now.X-last.X,now.Y-last.Y);matrix.Matrix=m;last=now;};
  view.MouseLeftButtonUp+=(s,e)=>{view.ReleaseMouseCapture();view.Cursor=null;};
  view.PreviewMouseWheel+=(s,e)=>{if((Keyboard.Modifiers&ModifierKeys.Control)!=0){zoom(e.Delta>0?1.2:1/1.2,e.GetPosition(view));e.Handled=true;}};
  return grid;
 }
 static string TraceKind(object action){
  switch(AlertValue(action,"id")){
   case "3":return Equals(Field(action,"outbound"),true)?"外へ通信":Equals(Field(action,"outbound"),false)?"外から受信":"通信";
   case "11":return "ファイル作成";case "22":return "名前解決";case "23":case "26":return "ファイル削除";default:return "イベント";
  }
 }
}
