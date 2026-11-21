# Runs in an STA so WPF can decode clipboard bitmaps into PNG.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName PresentationCore
$data = [Windows.Clipboard]::GetDataObject()
$png = $data.GetData('PNG', $false)
if ($png -is [IO.Stream]) {
    if ($png.CanSeek) { $png.Position = 0 }
    $frame = [Windows.Media.Imaging.BitmapFrame]::Create($png)
} else {
    $bitmap = [Windows.Clipboard]::GetImage()
    if (!$bitmap) { throw 'Clipboard image unavailable' }
    $frame = [Windows.Media.Imaging.BitmapFrame]::Create($bitmap)
}
$encoder = [Windows.Media.Imaging.PngBitmapEncoder]::new()
$encoder.Frames.Add($frame)
$file = [IO.File]::Create($env:KAGARI_CLIPBOARD_PNG)
try { $encoder.Save($file) } finally { $file.Dispose() }
