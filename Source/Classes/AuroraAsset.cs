// 
// 	AuroraAsset.cs
// 	AuroraAssetEditor
// 
// 	Created by Swizzy on 04/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Classes {
    using System;
    using System.Collections.Generic;
    using System.Drawing;
    using System.Drawing.Imaging;
    using System.Linq;
    using System.Runtime.InteropServices;
    using System.Text;
    using PhoenixTools;

    public static class AuroraAsset {
        public enum AssetType {
            Icon, // Icon
            Banner, // Banner
            Boxart, // Cover
            Slot, // NXEArt, currently not used
            Background, // Background
            ScreenshotStart, // Screenshot 1
            ScreenshotEnd = ScreenshotStart + ScreenShotMax, // Screenshot 20
            Max = ScreenshotEnd // End of it all
        }

        private const int ScreenShotMax = 20;

        private static uint Swap(uint x) { return (x & 0x000000FF) << 24 | (x & 0x0000FF00) << 8 | (x & 0x00FF0000) >> 8 | (x & 0xFF000000) >> 24; }

        private static Image RawArgbToImage(byte[] raw, int width, int height) {
            var ret = new Bitmap(width, height);
            var rect = new Rectangle(new Point(0, 0), new Size(width, height));
            var bmpData = ret.LockBits(rect, ImageLockMode.ReadWrite, ret.PixelFormat);
            Marshal.Copy(raw, 0, bmpData.Scan0, raw.Length);
            ret.UnlockBits(bmpData);
            return ret;
        }

        private static byte[] ImageToRawArgb(Image img) {
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

        public class AssetFile {
            public readonly int DataOffset;
            public readonly AssetPackEntryTable EntryTable;
            public readonly AssetPackHeader Header;

            public AssetFile() {
                Header = new AssetPackHeader(0x52584541, 1, 0);
                EntryTable = new AssetPackEntryTable();
                DataOffset = 20 + (EntryTable.Entries.Length * 64);
                DataOffset += 2048 - (DataOffset % 2048);
            }

            public AssetFile(byte[] data) {
                if(data == null) {
                    Header = new AssetPackHeader(0x52584541, 1, 0);
                    EntryTable = new AssetPackEntryTable();
                    DataOffset = 20 + (EntryTable.Entries.Length * 64);
                    DataOffset += 2048 - (DataOffset % 2048);
                    return;
                }
                if(data.Length < 2048)
                    throw new Exception("Invalid asset file size!");
                var magic = Swap(BitConverter.ToUInt32(data, 0));
                if(magic != 0x52584541)
                    throw new Exception("Invalid asset file magic!");
                var version = Swap(BitConverter.ToUInt32(data, 4));
                if(version != 1)
                    throw new NotSupportedException("Unsupported asset file version!");
                var datasize = Swap(BitConverter.ToUInt32(data, 8));
                Header = new AssetPackHeader(magic, version, datasize);
                EntryTable = new AssetPackEntryTable(data, 12);
                DataOffset = 20 + (EntryTable.Entries.Length * 64);
                DataOffset += 2048 - (DataOffset % 2048);
                var offset = DataOffset;
                for(var i = 0; i < EntryTable.Entries.Length; i++) {
                    if(EntryTable.Entries[i].Size <= 0)
                        continue;
                    var tmp = new byte[EntryTable.Entries[i].Size];
                    Buffer.BlockCopy(data, offset, tmp, 0, tmp.Length);
                    SetImage(tmp, i);
                    offset += tmp.Length;
                }
            }

            public int PaddingSize { get { return 0x800 - ((0x14 + EntryTable.Entries.Length * 0x40) % 0x800); } }

            public IEnumerable<byte> Padding { get { return new byte[PaddingSize]; } }

            public byte[] FileData {
                get {
                    var ret = new List<byte>();
                    uint offset = 0;
                    Header.DataSize = 0;
                    EntryTable.Flags = 0;
                    EntryTable.ScreenshotCount = 0;
                    for(var i = 0; i < EntryTable.Entries.Length; i++) {
                        var entry = EntryTable.Entries[i];
                        if(entry.Size <= 0)
                            continue;
                        entry.Offset = offset;
                        offset += entry.Size;
                        Header.DataSize += entry.Size;
                        EntryTable.Flags |= (uint)(1 << i);
                        if(i <= (int)AssetType.ScreenshotEnd && i >= (int)AssetType.ScreenshotStart)
                            EntryTable.ScreenshotCount++;
                    }
                    ret.AddRange(BitConverter.GetBytes(Swap(Header.Magic)));
                    ret.AddRange(BitConverter.GetBytes(Swap(Header.Version)));
                    ret.AddRange(BitConverter.GetBytes(Swap(Header.DataSize)));
                    ret.AddRange(BitConverter.GetBytes(Swap(EntryTable.Flags)));
                    ret.AddRange(BitConverter.GetBytes(Swap(EntryTable.ScreenshotCount)));
                    foreach(var entry in EntryTable.Entries) {
                        ret.AddRange(BitConverter.GetBytes(Swap(entry.Offset)));
                        ret.AddRange(BitConverter.GetBytes(Swap(entry.Size)));
                        ret.AddRange(BitConverter.GetBytes(Swap(entry.ExtendedInfo)));
                        ret.AddRange(entry.TextureHeader);
                    }
                    ret.AddRange(Padding);
                    foreach(var entry in EntryTable.Entries.Where(entry => entry.Size > 0))
                        ret.AddRange(entry.VideoData);
                    return ret.ToArray();
                }
            }

            public bool HasBoxArt { get { return EntryTable.Entries[(int)AssetType.Boxart].Size > 0; } }

            public bool HasBackground { get { return EntryTable.Entries[(int)AssetType.Background].Size > 0; } }

            public bool HasScreenshots { get { return EntryTable.ScreenshotCount > 0; } }

            public bool HasIconBanner { get { return EntryTable.Entries[(int)AssetType.Icon].Size > 0 || EntryTable.Entries[(int)AssetType.Banner].Size > 0; } }

            private bool SetImage(Image img, int index, bool useCompression) {
                if(index > (int)AssetType.Max)
                    return false;
                if(img == null) {
                    EntryTable.Entries[index].ImageData = null;
                    EntryTable.Entries[index].VideoData = new byte[0];
                    EntryTable.Entries[index].TextureHeader = new byte[EntryTable.Entries[index].TextureHeader.Length];
                    return true;
                }
                EntryTable.Entries[index].ImageData = img;
                var data = ImageToRawArgb(img);
                byte[] video = new byte[0], header = new byte[0];
                if(!AuroraAssetDll.ProcessImageToAsset(ref data, img.Width, img.Height, useCompression, ref header, ref video))
                    return false;
                EntryTable.Entries[index].VideoData = video;
                EntryTable.Entries[index].TextureHeader = header;
                return true;
            }

            private void SetImage(byte[] videoData, int index) {
                var imageData = new byte[0];
                int imageWidth, imageHeight;
                if(!AuroraAssetDll.ProcessAssetToImage(ref EntryTable.Entries[index].TextureHeader, ref videoData, ref imageData, out imageWidth, out imageHeight))
                    return;
                EntryTable.Entries[index].VideoData = videoData;
                EntryTable.Entries[index].ImageData = RawArgbToImage(imageData, imageWidth, imageHeight);
            }

            private void SetImage(AssetFile asset, int index) {
                var target = EntryTable.Entries[index];
                var src = asset.EntryTable.Entries[index];
                target.TextureHeader = src.TextureHeader;
                target.VideoData = src.VideoData;
                target.ImageData = src.ImageData;
            }

            public bool SetIcon(Image img, bool useCompression) { return SetImage(img, (int)AssetType.Icon, useCompression); }

            public bool SetBackground(Image img, bool useCompression) { return SetImage(img, (int)AssetType.Background, useCompression); }

            public bool SetBanner(Image img, bool useCompression) { return SetImage(img, (int)AssetType.Banner, useCompression); }

            public bool SetBoxart(Image img, bool useCompression) { return SetImage(img, (int)AssetType.Boxart, useCompression); }

            public bool SetScreenshot(Image img, int num, bool useCompression) {
                num += (int)AssetType.ScreenshotStart - 1;
                return num <= (int)AssetType.ScreenshotEnd && SetImage(img, num, useCompression);
            }

            public Image GetIcon() { return EntryTable.Entries[(int)AssetType.Icon].Size > 0 ? EntryTable.Entries[(int)AssetType.Icon].ImageData : null; }

            public Image GetBoxart() { return EntryTable.Entries[(int)AssetType.Boxart].Size > 0 ? EntryTable.Entries[(int)AssetType.Boxart].ImageData : null; }

            public Image GetBanner() { return EntryTable.Entries[(int)AssetType.Banner].Size > 0 ? EntryTable.Entries[(int)AssetType.Banner].ImageData : null; }

            public Image GetBackground() { return EntryTable.Entries[(int)AssetType.Background].Size > 0 ? EntryTable.Entries[(int)AssetType.Background].ImageData : null; }

            public Image GetScreenshot(int num) {
                num += (int)AssetType.ScreenshotStart - 1;
                if(num > (int)AssetType.ScreenshotEnd)
                    return null;
                return EntryTable.Entries[num].Size > 0 ? EntryTable.Entries[num].ImageData : null;
            }

            public Image[] GetScreenshots() {
                var ret = new List<Image>();
                for(var i = 0; i < ScreenShotMax; i++)
                    ret.Add(GetScreenshot(i + 1));
                return ret.ToArray();
            }

            public void SetBoxart(AssetFile asset) { SetImage(asset, (int)AssetType.Boxart); }

            public void SetBackground(AssetFile asset) { SetImage(asset, (int)AssetType.Background); }

            public void SetIcon(AssetFile asset) { SetImage(asset, (int)AssetType.Icon); }

            public void SetBanner(AssetFile asset) { SetImage(asset, (int)AssetType.Banner); }

            public void SetScreenshots(AssetFile asset) {
                for(var i = (int)AssetType.ScreenshotStart; i < (int)AssetType.ScreenshotEnd; i++)
                    SetImage(asset, i);
            }
        }

        public class AssetPackEntry {
            public Image ImageData;
            public byte[] TextureHeader;
            public byte[] VideoData;

            public AssetPackEntry() {
                Offset = 0;
                VideoData = new byte[0];
                TextureHeader = new byte[52];
            }

            public AssetPackEntry(byte[] data, int offset) {
                Offset = Swap(BitConverter.ToUInt32(data, offset));
                VideoData = new byte[Swap(BitConverter.ToUInt32(data, offset + 4))];
                TextureHeader = new byte[52];
                Buffer.BlockCopy(data, (offset + 12), TextureHeader, 0, TextureHeader.Length);
            }

            public uint Offset { get; internal set; }

            public uint Size { get { return (uint)VideoData.Length; } }

            public uint ExtendedInfo { get { return 0; } }

            public Size ImageSize { get { return ImageData.Size; } }

            public override string ToString() {
                var sz = ImageSize;
                return string.Format("Offset: 0x{0:X}{4}Size: 0x{1:X}{4}Extended Info: 0x{2:X}{4}Texture Header Size: 0x{3:X}{4}Width: {5}{4}Height: {6}", Offset, Size, ExtendedInfo,
                                     TextureHeader.Length, Environment.NewLine, sz.Width, sz.Height);
            }
        }

        public class AssetPackEntryTable {
            public readonly AssetPackEntry[] Entries = new AssetPackEntry[(int)AssetType.Max];

            public AssetPackEntryTable() {
                Flags = 0;
                ScreenshotCount = 0;
                for(var i = 0; i < Entries.Length; i++)
                    Entries[i] = new AssetPackEntry();
            }

            public AssetPackEntryTable(byte[] data, int offset) {
                Flags = Swap(BitConverter.ToUInt32(data, offset));
                ScreenshotCount = Swap(BitConverter.ToUInt32(data, offset + 4));
                offset += 8;
                for(var i = 0; i < Entries.Length; i++, offset += 64)
                    Entries[i] = new AssetPackEntry(data, offset);
            }

            public uint Flags { get; internal set; }

            public uint ScreenshotCount { get; internal set; }

            public override string ToString() {
                var sb = new StringBuilder();
                sb.AppendFormat("Flags: 0x{0:X}{1}", Flags, Environment.NewLine);
                sb.AppendFormat("ScreenshotCount: {0}{1}", ScreenshotCount, Environment.NewLine);
                sb.AppendLine("Entries:");
                for(var i = 0; i < Entries.Length; i++) {
                    if(i < (int)AssetType.ScreenshotStart || i > (int)AssetType.ScreenshotEnd)
                        sb.AppendLine(((AssetType)i) + ":");
                    else
                        sb.AppendLine(string.Format("ScreenShot {0}:", i - (int)AssetType.ScreenshotStart));
                    sb.AppendLine(Entries[i].Size > 0 ? Entries[i].ToString() : "No data...");
                }
                return sb.ToString();
            }
        }

        public class AssetPackHeader {
            public AssetPackHeader(uint magic, uint version, uint dataSize) {
                Magic = magic;
                Version = version;
                DataSize = dataSize;
            }

            public uint Magic { get; private set; }

            public uint Version { get; private set; }

            public uint DataSize { get; internal set; }

            public override string ToString() {
                return string.Format("Magic: {0}{3}Version: {1}{3}DataSize: {2}", Encoding.ASCII.GetString(BitConverter.GetBytes(Swap(Magic))), Version, DataSize, Environment.NewLine);
            }
        }
    }
}