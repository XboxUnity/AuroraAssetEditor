// 
// 	AuroraDbManager.cs
// 	AuroraAssetEditor
// 
// 	Created by Swizzy on 14/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Classes {
    using System;
    using System.Collections.Generic;
    using System.Data;
    using System.Data.SQLite;
    using System.Globalization;
    using System.IO;
    using System.Linq;
    using System.Threading;

    internal static class AuroraDbManager {
        private static SQLiteConnection _content;

        private static void ConnectToContent(string path) {
            if(_content != null)
                _content.Close();
            _content = new SQLiteConnection("Data Source=\"" + path + "\";Version=3;");
            _content.Open();
        }

        private static DataTable GetContentDataTable(string sql) {
            var dt = new DataTable();
            try {
                var cmd = new SQLiteCommand(sql, _content);
                using(var reader = cmd.ExecuteReader())
                    dt.Load(reader);
            }
            catch(Exception ex) {
                MainWindow.SaveError(ex);
            }
            return dt;
        }

        public static IEnumerable<ContentItem> GetDbTitles(string path) {
            ConnectToContent(path);
            var ret = GetContentItems().Select(item => item).ToList();
            _content.Close();
            GC.Collect();
            while(true) {
                try {
                    File.Delete(path);
                    break;
                }
                catch(IOException) {
                    Thread.Sleep(100);
                }
            }
            return ret;
        }

        private static IEnumerable<ContentItem> GetContentItems() { return GetContentDataTable("SELECT * FROM ContentItems").Select().Select(row => new ContentItem(row)).ToArray(); }

        internal class ContentItem {
            public ContentItem(DataRow row) {
                DatabaseId = ((int)((long)row["Id"])).ToString("X08");
                TitleId = ((int)((long)row["TitleId"])).ToString("X08");
                MediaId = ((int)((long)row["MediaId"])).ToString("X08");
                var discNum = (int)((long)row["DiscNum"]);
                if(discNum <= 0)
                    discNum = 1;
                DiscNum = discNum.ToString(CultureInfo.InvariantCulture);
                TitleName = (string)row["TitleName"];
            }

            public string TitleId { get; private set; }

            public string MediaId { get; private set; }

            public string DiscNum { get; private set; }

            public string TitleName { get; private set; }

            public string DatabaseId { get; private set; }

            public string Path { get { return string.Format("{0}_{1}", TitleId, DatabaseId); } }

            public void SaveAsBoxart(byte[] data) { App.FtpOperations.SendAssetData(string.Format("GC{0}.asset", TitleId), Path, data); }

            public void SaveAsBackground(byte[] data) { App.FtpOperations.SendAssetData(string.Format("BK{0}.asset", TitleId), Path, data); }

            public void SaveAsIconBanner(byte[] data) { App.FtpOperations.SendAssetData(string.Format("GL{0}.asset", TitleId), Path, data); }

            public void SaveAsScreenshots(byte[] data) { App.FtpOperations.SendAssetData(string.Format("SS{0}.asset", TitleId), Path, data); }

            public byte[] GetBoxart() { return App.FtpOperations.GetAssetData(string.Format("GC{0}.asset", TitleId), Path); }

            public byte[] GetBackground() { return App.FtpOperations.GetAssetData(string.Format("BK{0}.asset", TitleId), Path); }

            public byte[] GetIconBanner() { return App.FtpOperations.GetAssetData(string.Format("GL{0}.asset", TitleId), Path); }

            public byte[] GetScreenshots() { return App.FtpOperations.GetAssetData(string.Format("SS{0}.asset", TitleId), Path); }
        }
    }
}