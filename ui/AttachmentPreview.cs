using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;

internal sealed partial class Desktop {
 [UnmanagedFunctionPointer(CallingConvention.Winapi, CharSet=CharSet.Unicode)]
 delegate int PreviewCallback(IntPtr owner, IntPtr host, [MarshalAs(UnmanagedType.LPWStr)] string paths);
 [DllImport("kagari_core")] static extern void kagari_attachment_preview(PreviewCallback callback);
 static readonly PreviewCallback attachmentPreview=UpdateAttachmentPreview;
 static readonly Dictionary<IntPtr,HwndSource> previewHosts=new Dictionary<IntPtr,HwndSource>();
 [DllImport("user32.dll")] static extern IntPtr SendMessageW(IntPtr hwnd,uint message,IntPtr index,IntPtr data);

 static BitmapImage ReadAttachment(string path,int width){
  var bitmap=new BitmapImage();
  using(var stream=File.OpenRead(path)){
   bitmap.BeginInit();
   bitmap.CacheOption=BitmapCacheOption.OnLoad;
   bitmap.StreamSource=stream;
   bitmap.DecodePixelWidth=width;
   bitmap.EndInit();
  }
  bitmap.Freeze();
  return bitmap;
 }

 static void OpenAttachment(IntPtr owner,string path){
  try{
   var view=new Window{Title=Path.GetFileName(path)+" — 添付画像",Width=880,Height=640,
    WindowStartupLocation=WindowStartupLocation.CenterOwner,Background=Brushes.White,
    Content=new Image{Source=ReadAttachment(path,0),Stretch=Stretch.Uniform,Margin=new Thickness(8)}};
   new WindowInteropHelper(view).Owner=owner;
   view.ShowDialog();
  }catch(Exception err){MessageBox.Show("画像を開けませんでした。\n"+err.Message,"添付画像");}
 }

 static int UpdateAttachmentPreview(IntPtr owner,IntPtr host,string paths){
  try{
   HwndSource source;
   previewHosts.TryGetValue(host,out source);
   if(String.IsNullOrEmpty(paths)){
    if(source!=null)source.Dispose();
    return 0;
   }
   if(source==null){
    var parameters=new HwndSourceParameters("添付画像プレビュー"){ParentWindow=host,
     WindowStyle=unchecked((int)0x50000000),Width=1,Height=1};
    source=new HwndSource(parameters);
    source.SizeToContent=SizeToContent.Manual;
    previewHosts.Add(host,source);
    source.Disposed+=(s,e)=>previewHosts.Remove(host);
    var row=new StackPanel{Orientation=Orientation.Horizontal,Background=Brushes.White};
    source.RootVisual=new ScrollViewer{Content=row,Background=Brushes.White,
     HorizontalScrollBarVisibility=ScrollBarVisibility.Auto,VerticalScrollBarVisibility=ScrollBarVisibility.Disabled};
   }
   var thumbnails=(StackPanel)((ScrollViewer)source.RootVisual).Content;
   var files=paths.Split('\n');
   for(int i=thumbnails.Children.Count-1;i>=0;i--){
    var tile=(FrameworkElement)thumbnails.Children[i];
    if(Array.IndexOf(files,(string)tile.Tag)<0)thumbnails.Children.RemoveAt(i);
   }
   for(int i=thumbnails.Children.Count;i<files.Length;i++){
    string file=files[i],name=Path.GetFileName(file);
    var button=new Button{Width=84,Padding=new Thickness(2),Margin=new Thickness(1),
     ToolTip=name+" を拡大",Background=Brushes.White};
    try{button.Content=new Image{Source=ReadAttachment(file,160),Stretch=Stretch.Uniform};}
    catch(Exception){button.Content=new TextBlock{Text=name,TextTrimming=TextTrimming.CharacterEllipsis};}
    AutomationProperties.SetName(button,"添付画像を開く　"+name);
    button.Click+=(s,e)=>OpenAttachment(owner,file);
    var tile=new StackPanel{Orientation=Orientation.Horizontal,Tag=file};
    tile.Children.Add(button);
    var remove=new Button{Content="×",MinWidth=26,ToolTip=name+" を削除",Margin=new Thickness(0,1,3,1)};
    AutomationProperties.SetName(remove,"添付画像を削除　"+name);
    remove.Click+=(s,e)=>{
     int index=thumbnails.Children.IndexOf(tile);
     if(index>=0)SendMessageW(owner,0x8000+113,new IntPtr(index),IntPtr.Zero);
    };
    tile.Children.Add(remove);
    thumbnails.Children.Add(tile);
   }
   return 1;
  }catch(Exception){return 0;}
 }
}
