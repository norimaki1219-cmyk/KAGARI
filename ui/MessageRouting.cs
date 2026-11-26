using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using Microsoft.Win32;

internal static class MessageRouting {
    public sealed class Rule {
        public string source { get; set; }
        public string sender { get; set; }
        public string contains { get; set; }
        public string project { get; set; }
    }
    static readonly JavaScriptSerializer Json = new JavaScriptSerializer();

    static string ProfileFile(string project) {
        var path = Path.GetFullPath(project).TrimEnd('\\');
        var key = path.StartsWith(@"\\") ? @"\\?\UNC\" + path.Substring(2) : @"\\?\" + path;
        using (var sha = SHA256.Create()) {
            var hash = sha.ComputeHash(Encoding.UTF8.GetBytes(key.ToLowerInvariant()));
            return Path.Combine(Environment.GetEnvironmentVariable("LOCALAPPDATA"), "RustAIWorkspace",
                "message-routing", BitConverter.ToString(hash).Replace("-", "").ToLowerInvariant() + ".json");
        }
    }

    static string Registered(string path, string[] projects) {
        if (path == null) return null;
        if (path.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase)) path = @"\\" + path.Substring(8);
        else if (path.StartsWith(@"\\?\")) path = path.Substring(4);
        return projects.FirstOrDefault(p => String.Equals(p.TrimEnd('\\'), path.Replace('/', '\\').TrimEnd('\\'), StringComparison.OrdinalIgnoreCase));
    }

    static Rule[] ReadRules(object raw, string[] projects) {
        var array = raw as object[];
        if (array == null) throw new InvalidDataException("ルールの配列が必要です。");
        return array.Select((entry, i) => {
            var fields = entry as Dictionary<string, object>;
            if (fields == null || fields.Any(f => !(f.Value is string) ||
                !new[]{"source","sender","contains","project"}.Contains(f.Key)) ||
                !fields.ContainsKey("project") || !(fields.ContainsKey("sender") || fields.ContainsKey("contains")))
                throw new InvalidDataException((i+1) + " 行目のルールが無効です。");
            var rule = Json.ConvertToType<Rule>(fields);
            rule.project = Registered(rule.project, projects);
            return rule;
        }).ToArray();
    }

    internal static void Open(Window owner, string project, string[] projects) {
        try { Edit(owner, project, projects); }
        catch (Exception ex) { MessageBox.Show(owner, ex.Message, "外部連絡の取込設定"); }
    }

    static Button ActionButton(Panel panel, string title, Action action) {
        var button = new Button {Content=title, Margin=new Thickness(3), Padding=new Thickness(9,4,9,4)};
        button.Click += (s,e) => action();
        panel.Children.Add(button);
        return button;
    }

    static void Edit(Window owner, string project, string[] projects) {
        var file = ProfileFile(project);
        string original = File.Exists(file) ? File.ReadAllText(file) : null;
        var rows = new ObservableCollection<Rule>();
        var destination = new ComboBox {ItemsSource=projects, SelectedItem=project, Margin=new Thickness(0,4,0,12)};
        if (original != null) {
            var profile = Json.DeserializeObject(original) as Dictionary<string,object>;
            if(profile==null || !profile.ContainsKey("defaultProject") || !profile.ContainsKey("rules"))
                throw new InvalidDataException("外部連絡の取込設定が無効です。");
            destination.SelectedItem = Registered((string)profile["defaultProject"], projects);
            foreach (var rule in ReadRules(profile["rules"], projects)) rows.Add(rule);
        }
        var dialog = new Window {Owner=owner, Title="外部連絡の取込設定", Width=850, Height=490,
            MinWidth=640, MinHeight=380, WindowStartupLocation=WindowStartupLocation.CenterOwner};
        var layout = new DockPanel {Margin=new Thickness(16)};
        dialog.Content = layout;
        var header = new StackPanel();
        DockPanel.SetDock(header, Dock.Top); layout.Children.Add(header);
        header.Children.Add(new TextBlock {Text="MCP の接続元プロジェクト　" + project, TextWrapping=TextWrapping.Wrap});
        header.Children.Add(new TextBlock {Text="一致するルールがない場合の登録先", Margin=new Thickness(0,12,0,0)});
        header.Children.Add(destination);
        header.Children.Add(new TextBlock {Text="上から順に判定します。差出人（完全一致）とキーワード（部分一致）は片方または両方を指定してください。",TextWrapping=TextWrapping.Wrap});
        var footer = new StackPanel();
        DockPanel.SetDock(footer, Dock.Bottom); layout.Children.Add(footer);
        var error = new TextBlock {TextWrapping=TextWrapping.Wrap, Foreground=System.Windows.Media.Brushes.DarkRed};
        footer.Children.Add(error);
        var buttons = new WrapPanel();
        footer.Children.Add(buttons);
        var grid = new DataGrid {ItemsSource=rows, AutoGenerateColumns=false, CanUserAddRows=false,
            CanUserDeleteRows=false, CanUserSortColumns=false, SelectionMode=DataGridSelectionMode.Single};
        layout.Children.Add(grid);
        foreach (var field in new[]{new[]{"サービス","source"},new[]{"差出人","sender"},new[]{"キーワード","contains"}})
            grid.Columns.Add(new DataGridTextColumn {Header=field[0], Binding=new Binding(field[1]), Width=new DataGridLength(1,DataGridLengthUnitType.Star)});
        grid.Columns.Add(new DataGridComboBoxColumn {Header="登録先", ItemsSource=projects, SelectedItemBinding=new Binding("project"),
            Width=new DataGridLength(2,DataGridLengthUnitType.Star)});
        Action commit = () => {grid.CommitEdit(DataGridEditingUnit.Cell,true); grid.CommitEdit(DataGridEditingUnit.Row,true);};
        ActionButton(buttons,"追加",()=>{commit(); var row=new Rule {project=destination.SelectedItem as string}; rows.Add(row); grid.SelectedItem=row; grid.ScrollIntoView(row);});
        ActionButton(buttons,"削除",()=>{commit(); if(grid.SelectedIndex>=0) rows.RemoveAt(grid.SelectedIndex);});
        Action<int> move = delta => {commit(); int i=grid.SelectedIndex; if(i>=0 && i+delta>=0 && i+delta<rows.Count) {rows.Move(i,i+delta); grid.SelectedIndex=i+delta;}};
        ActionButton(buttons,"上へ",()=>move(-1)); ActionButton(buttons,"下へ",()=>move(1));
        ActionButton(buttons,"旧ルールを追加…",()=>{
            var picker=new OpenFileDialog {Filter="JSON (*.json)|*.json|すべて (*.*)|*.*"};
            if(picker.ShowDialog(dialog)!=true) return;
            try {var imported=ReadRules(Json.DeserializeObject(File.ReadAllText(picker.FileName)),projects);
                commit(); foreach(var rule in imported) rows.Add(rule); error.Text="";}
            catch(Exception ex){error.Text=ex.Message;}
        });
        ActionButton(buttons,"保存",()=>{
            try {
                commit();
                if(destination.SelectedItem==null) throw new InvalidDataException("既定の登録先を選択してください。");
                var rules=new List<Dictionary<string,string>>();
                foreach(var rule in rows) {
                    if(rule.project==null || (String.IsNullOrWhiteSpace(rule.sender) && String.IsNullOrWhiteSpace(rule.contains)))
                        throw new InvalidDataException((rules.Count+1)+" 行目の登録先と差出人またはキーワードを指定してください。");
                    var item=new Dictionary<string,string>{{"project",rule.project}};
                    if(!String.IsNullOrWhiteSpace(rule.source)) item.Add("source",rule.source.Trim());
                    if(!String.IsNullOrWhiteSpace(rule.sender)) item.Add("sender",rule.sender.Trim());
                    if(!String.IsNullOrWhiteSpace(rule.contains)) item.Add("contains",rule.contains.Trim());
                    rules.Add(item);
                }
                Directory.CreateDirectory(Path.GetDirectoryName(file));
                using(new FileStream(file+".lock",FileMode.OpenOrCreate,FileAccess.Write,FileShare.None)) {
                    if((File.Exists(file)?File.ReadAllText(file):null)!=original)
                        throw new IOException("別の画面で設定が更新されました。開き直してください。");
                    var temp=file+"."+Guid.NewGuid().ToString("N")+".tmp";
                    try {
                        File.WriteAllText(temp,Json.Serialize(new {defaultProject=destination.SelectedItem,rules=rules}));
                        if(original==null) File.Move(temp,file); else File.Replace(temp,file,null);
                    } finally {if(File.Exists(temp)) File.Delete(temp);}
                }
                dialog.DialogResult=true;
            } catch(Exception ex){error.Text=ex.Message;}
        });
        ActionButton(buttons,"キャンセル",()=>dialog.Close()).IsCancel=true;
        dialog.ShowDialog();
    }
}
