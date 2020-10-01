// 
// 	FSDAsset.cs
// 	AuroraAssetEditor
// 
// 	Created by Swizzy on 10/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Classes {
    using System;
    using System.Drawing;
    using System.Drawing.Imaging;
    using System.IO;
    using System.Linq;
    using System.Runtime.InteropServices;
    using PhoenixTools;

    internal class FsdAsset {
        public enum FsdAssetType: uint {
            Thumbnail = 0x01,
            Background = 0x02,
            Banner = 0x04,
            Boxart = 0x08,
            Preview = 0x10,
            Screenshot = 0x20,
            Slot = 0x40,
            FullCover = 0x80
        }

        public readonly uint AssetCount;
        public readonly uint AssetFlags;
        public readonly FsdAssetEntry[] Entries = new FsdAssetEntry[10];
        public readonly uint Magic;
        public readonly uint Reserved;
        public readonly uint ScreenshotCount;
        public readonly FsdScreenshotEntry[] Screenshots = new FsdScreenshotEntry[20];
        public readonly uint Version;

        public FsdAsset(byte[] data) {
            if(data.Length < 0x1F8)
                throw new Exception("Invalid file size");
            Magic = Swap(BitConverter.ToUInt32(data, 0));
            if(Magic != 0x46534441)
                throw new Exception("Invalid asset file magic!");
            Version = Swap(BitConverter.ToUInt32(data, 4));
            if(Version != 1)
                throw new NotSupportedException("Unsupported asset file version!");
            Reserved = Swap(BitConverter.ToUInt32(data, 8));
            AssetFlags = Swap(BitConverter.ToUInt32(data, 12));
            AssetCount = Swap(BitConverter.ToUInt32(data, 16));
            ScreenshotCount = Swap(BitConverter.ToUInt32(data, 20));
            for(var i = 0; i < Entries.Length; i++)
                Entries[i] = new FsdAssetEntry(ref data, 24 + (i * 16));
            for(var i = 0; i < Screenshots.Length; i++)
                Screenshots[i] = new FsdScreenshotEntry(ref data, (24 + (16 * Entries.Length)) + (i * 16));
        }

        private static uint Swap(uint x) { return (x & 0x000000FF) << 24 | (x & 0x0000FF00) << 8 | (x & 0x00FF0000) >> 8 | (x & 0xFF000000) >> 24; }

        private static Image RawArgbToImage(byte[] raw, int width, int height) {
            var ret = new Bitmap(width, height);
            var rect = new Rectangle(new Point(0, 0), new Size(width, height));
            var bmpData = ret.LockBits(rect, ImageLockMode.ReadWrite, ret.PixelFormat);
            Marshal.Copy(raw, 0, bmpData.Scan0, raw.Length);
            ret.UnlockBits(bmpData);
            return ret;
        }

        private static Image GetImage(byte[] data) {
            if(data[0] == 'D' || data[1] == 'D' || data[1] == 'S') {
                var imageData = new byte[0];
                int imageWidth, imageHeight;
                AuroraAssetDll.ProcessDDSToImage(ref data, ref imageData, out imageWidth, out imageHeight);
                return RawArgbToImage(imageData, imageWidth, imageHeight);
            }
            using(var ms = new MemoryStream(data)) {
                var img = Image.FromStream(ms);
                var img2 = new Bitmap(img.Width, img.Height, PixelFormat.Format32bppArgb);
                var g = Graphics.FromImage(img);
                g.DrawImage(img2, new Point(0, 0));
                g.Dispose();
                img.Dispose();
                return img2;
            }
        }

        public Image GetBanner() { return (from entry in Entries where entry.AssetType == FsdAssetType.Banner && entry.Size > 0 select GetImage(entry.Data)).FirstOrDefault(); }

        public Image GetBackground() { return (from entry in Entries where entry.AssetType == FsdAssetType.Background && entry.Size > 0 select GetImage(entry.Data)).FirstOrDefault(); }

        public Image GetIcon() { return (from entry in Entries where entry.AssetType == FsdAssetType.Thumbnail && entry.Size > 0 select GetImage(entry.Data)).FirstOrDefault(); }

        public Image GetBoxart() { return (from entry in Entries where entry.AssetType == FsdAssetType.FullCover && entry.Size > 0 select GetImage(entry.Data)).FirstOrDefault(); }

        public Image[] GetScreenshots() {
            var ret = (from entry in Screenshots where entry.Size > 0 select GetImage(entry.Data)).ToList();
            if(ret.Count == 0)
                ret.AddRange(from entry in Entries where entry.AssetType == FsdAssetType.Screenshot && entry.Size > 0 select GetImage(entry.Data));
            return ret.ToArray();
        }

        public class FsdAssetEntry {
            public readonly FsdAssetType AssetType;
            public readonly byte[] Data;
            public readonly uint Offset;
            public readonly uint Size;
            public readonly uint TotalSize;

            public FsdAssetEntry(ref byte[] data, int offset) {
                AssetType = (FsdAssetType)Swap(BitConverter.ToUInt32(data, offset));
                Offset = Swap(BitConverter.ToUInt32(data, offset + 4));
                Size = Swap(BitConverter.ToUInt32(data, offset + 8));
                TotalSize = Swap(BitConverter.ToUInt32(data, offset + 12));
                if(Size <= 0)
                    return;
                Data = new byte[Size];
                Buffer.BlockCopy(data, (int)Offset, Data, 0, Data.Length);
            }
        }

        public class FsdScreenshotEntry {
            public readonly byte[] Data;
            public readonly uint Offset;
            public readonly uint Reserved;
            public readonly uint Size;
            public readonly uint TotalSize;

            public FsdScreenshotEntry(ref byte[] data, int offset) {
                Offset = Swap(BitConverter.ToUInt32(data, offset));
                Size = Swap(BitConverter.ToUInt32(data, offset + 4));
                TotalSize = Swap(BitConverter.ToUInt32(data, offset + 8));
                Reserved = Swap(BitConverter.ToUInt32(data, offset + 12));
                if(Size <= 0)
                    return;
                Data = new byte[Size];
                Buffer.BlockCopy(data, (int)Offset, Data, 0, Data.Length);
            }
        }
    }
}