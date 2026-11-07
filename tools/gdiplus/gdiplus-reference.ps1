<#
Writes GDI+ reference pixels for the GDI+ closeness report (spec 6.2): every input goes
through the old app's image path, MainWindow.GetImage followed by AuroraAsset.ImageToRawArgb,
and comes out as the A, R, G, B bytes the old app handed to AuroraAsset.dll.

Requires Windows PowerShell 5.1 (powershell.exe, .NET Framework 4.x, the old app's net48
runtime). The old app ran as a 32-bit process (Prefer32Bit), so run the 32-bit host:
C:\Windows\SysWOW64\WindowsPowerShell\v1.0\powershell.exe.

For an input already at one of the target sizes only that size is written; any other input
is written at every target, as <name>.<width>x<height>.argb in the output folder.
#>
param(
    [Parameter(Mandatory = $true)][string]$InputDir,
    [Parameter(Mandatory = $true)][string]$OutputDir,
    [string]$Targets = "900x600,1280x720,64x64,420x96,1000x562"
)

$ErrorActionPreference = "Stop"

Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System.Drawing;
using System.IO;

public static class OldImagePath {
    // MainWindow.GetImage with Auto Resize on.
    public static Image GetImage(string filename, Size newSize) {
        var ms = new MemoryStream(File.ReadAllBytes(filename));
        var img = Image.FromStream(ms);
        if(!img.Size.Equals(newSize)) {
            img = new Bitmap(img, newSize);
        }
        return img;
    }

    // AuroraAsset.ImageToRawArgb.
    public static byte[] ImageToRawArgb(Image img) {
        var bmp = new Bitmap(img);
        var ret = new byte[bmp.Height * bmp.Width * 4];
        var i = 0;
        for(var y = 0; y < bmp.Height; y++) {
            for(var x = 0; x < bmp.Width; x++) {
                var c = bmp.GetPixel(x, y);
                ret[i] = c.A;
                ret[i + 1] = c.R;
                ret[i + 2] = c.G;
                ret[i + 3] = c.B;
                i += 4;
            }
        }
        return ret;
    }

    public static Size SizeOf(string filename) {
        using(var ms = new MemoryStream(File.ReadAllBytes(filename))) {
            using(var img = Image.FromStream(ms)) {
                return img.Size;
            }
        }
    }
}
"@

function ConvertTo-Size([string]$text) {
    $parts = $text.Trim().Split("x")
    New-Object System.Drawing.Size([int]$parts[0], [int]$parts[1])
}

$targetSizes = @($Targets.Split(",") | ForEach-Object { ConvertTo-Size $_ })
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

foreach ($file in Get-ChildItem -File -Path $InputDir | Sort-Object Name) {
    $size = [OldImagePath]::SizeOf($file.FullName)
    $sizes = @($targetSizes | Where-Object { $_.Equals($size) })
    if ($sizes.Count -eq 0) {
        $sizes = $targetSizes
    }
    foreach ($target in $sizes) {
        $image = [OldImagePath]::GetImage($file.FullName, $target)
        $argb = [OldImagePath]::ImageToRawArgb($image)
        $image.Dispose()
        $name = "{0}.{1}x{2}.argb" -f $file.BaseName, $target.Width, $target.Height
        [System.IO.File]::WriteAllBytes((Join-Path $OutputDir $name), $argb)
        Write-Output $name
    }
}
