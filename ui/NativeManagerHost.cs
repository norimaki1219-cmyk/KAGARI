using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

// The existing Tauri webview owns the model session and its IPC. Borrow its manager
// window while this pane is visible and restore its original desktop placement on exit.
internal sealed class NativeManagerHost:HwndHost {
 const int StyleIndex=-16;
 const long Child=0x40000000, Popup=0x80000000, Chrome=0x00CF0000;
 [StructLayout(LayoutKind.Sequential)] struct Rect {public int left,top,right,bottom;}
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateWindowEx(int ex,string cls,string title,int style,int x,int y,int w,int h,IntPtr parent,IntPtr menu,IntPtr instance,IntPtr param);
 [DllImport("user32.dll")] static extern bool DestroyWindow(IntPtr h);
 [DllImport("user32.dll")] static extern bool IsWindow(IntPtr h);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] static extern IntPtr GetWindowLongPtrW(IntPtr h,int index);
 [DllImport("user32.dll",SetLastError=true)] static extern IntPtr SetWindowLongPtrW(IntPtr h,int index,IntPtr style);
 [DllImport("user32.dll",SetLastError=true)] static extern IntPtr SetParent(IntPtr h,IntPtr parent);
 [DllImport("kernel32.dll")] static extern void SetLastError(uint error);
 [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr h,int cmd);
 [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h,IntPtr order,int x,int y,int w,int height,uint flags);
 IntPtr site,manager,oldParent,oldStyle;
 Rect placement;bool wasVisible;
 protected override HandleRef BuildWindowCore(HandleRef parent){
  site=CreateWindowEx(0,"STATIC","",0x56000000,0,0,1,1,parent.Handle,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero);
  if(site==IntPtr.Zero)throw new Win32Exception();
  return new HandleRef(this,site);
 }
 internal void Attach(IntPtr hwnd){
  if(site==IntPtr.Zero)throw new InvalidOperationException("管理ペインを初期化できませんでした。");
  oldStyle=GetWindowLongPtrW(hwnd,StyleIndex);GetWindowRect(hwnd,out placement);wasVisible=IsWindowVisible(hwnd);
  long style=(oldStyle.ToInt64() & ~(Chrome|Popup))|Child;
  if(SetWindowLongPtrW(hwnd,StyleIndex,new IntPtr(style))==IntPtr.Zero)throw new Win32Exception();
  SetLastError(0);oldParent=SetParent(hwnd,site);
  int error=Marshal.GetLastWin32Error();
  if(oldParent==IntPtr.Zero && error!=0){SetWindowLongPtrW(hwnd,StyleIndex,oldStyle);throw new Win32Exception(error);}
  manager=hwnd;ResizeChild();ShowWindow(manager,5);
 }
 void ResizeChild(){Rect r;if(manager!=IntPtr.Zero && GetClientRect(site,out r))SetWindowPos(manager,IntPtr.Zero,0,0,Math.Max(1,r.right),Math.Max(1,r.bottom),0x34);}
 protected override IntPtr WndProc(IntPtr hwnd,int msg,IntPtr w,IntPtr l,ref bool handled){
  if(msg==5)ResizeChild();
  return base.WndProc(hwnd,msg,w,l,ref handled);
 }
 protected override void DestroyWindowCore(HandleRef host){
  if(manager!=IntPtr.Zero && IsWindow(manager)){
   ShowWindow(manager,0);
   SetParent(manager,oldParent);SetWindowLongPtrW(manager,StyleIndex,oldStyle);
   SetWindowPos(manager,IntPtr.Zero,placement.left,placement.top,placement.right-placement.left,placement.bottom-placement.top,0x34);
   if(wasVisible)ShowWindow(manager,8);
  }
  manager=IntPtr.Zero;DestroyWindow(host.Handle);site=IntPtr.Zero;
 }
}
