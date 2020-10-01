//
// 	XboxAssetDownloader.cs
// 	AuroraAssetEditor
//
// 	Created by Swizzy on 10/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Classes {
    using System;
    using System.Collections.Generic;
    using System.Drawing;
    using System.Globalization;
    using System.IO;
    using System.Linq;
    using System.Net;
    using System.Runtime.Serialization;
    using System.Runtime.Serialization.Json;
    using System.Text;
    using System.Web;
    using System.Xml;

    internal class XboxAssetDownloader {
        public static EventHandler<StatusArgs> StatusChanged;
        private readonly DataContractJsonSerializer _serializer = new DataContractJsonSerializer(typeof(XboxKeywordResponse));

        internal static void SendStatusChanged(string msg) {
            var handler = StatusChanged;
            if(handler != null)
                handler.Invoke(null, new StatusArgs(msg));
        }

        public XboxTitleInfo[] GetTitleInfo(uint titleId, XboxLocale locale) {
            return new[] {
                             XboxTitleInfo.FromTitleId(titleId, locale)
                         };
        }

        public XboxTitleInfo[] GetTitleInfo(string keywords, XboxLocale locale) {
            var url = string.Format("http://marketplace.xbox.com/{0}/SiteSearch/xbox/?query={1}&PageSize=5", locale.Locale, HttpUtility.UrlEncode(keywords));
            var wc = new WebClient();
            var ret = new List<XboxTitleInfo>();
            using(var stream = wc.OpenRead(url)) {
                if(stream == null)
                    return ret.ToArray();
                var res = (XboxKeywordResponse)_serializer.ReadObject(stream);
                ret.AddRange(from entry in res.Entries where entry.DetailsUrl != null let tid = entry.DetailsUrl.IndexOf("d802", StringComparison.Ordinal) where tid > 0 && entry.DetailsUrl.Length >= tid + 12 select uint.Parse(entry.DetailsUrl.Substring(tid + 4, 8), NumberStyles.HexNumber) into titleId select XboxTitleInfo.FromTitleId(titleId, locale));
            }
            return ret.ToArray();
        }

        public static XboxLocale[] GetLocales() {
            try {
            var ret = new List<XboxLocale>();
            var tmp = new List<string>();
            var wc = new WebClient();
            var data = Encoding.UTF8.GetString(wc.DownloadData("http://www.xbox.com/Shell/ChangeLocale")).Split('>');
            for(var i = 0; i < data.Length; i++) {
                if(!data[i].ToLower().Contains("?source=lp"))
                    continue;
                var index = data[i].ToLower().IndexOf("\"pid\":", StringComparison.Ordinal) + 7;
                var id = data[i].Substring(index);
                index = id.IndexOf('"');
                if(index <= 0)
                    continue;
                id = id.Substring(0, index);
                if(tmp.Contains(id))
                    continue;
                var name = data[i + 1];
                name = name.Substring(0, name.IndexOf("</a", StringComparison.Ordinal));
                name = name.Trim('\r','\n',' ');
                name = HttpUtility.HtmlDecode(name);
                ret.Add(new XboxLocale(id, name));
                tmp.Add(id);
            }
            ret.Sort((l1, l2) => string.CompareOrdinal(l1.ToString(), l2.ToString()));
            return ret.ToArray();
            }
            catch { return new XboxLocale[0]; }
        }
    }

    public class XboxTitleInfo {
        public enum XboxAssetType {
            Icon,
            Banner,
            Background,
            Screenshot
        }

        public string Title { get; private set; }

        public string TitleId { get; private set; }

        public string Locale { get; private set; }

        public XboxAssetInfo[] AssetsInfo { get; private set; }

        public XboxAsset[] Assets {
            get {
                if(AssetsInfo.Any(info => !info.HaveAsset))
                    XboxAssetDownloader.SendStatusChanged(string.Format("Downloading assets for {0}...", Title));
                var ret = AssetsInfo.Select(info => info.GetAsset()).ToArray();
                return ret;
            }
        }

        public XboxAsset[] IconAssets {
            get {
                if(AssetsInfo.Where(info => info.AssetType == XboxAssetType.Icon).Any(info => !info.HaveAsset))
                    XboxAssetDownloader.SendStatusChanged(string.Format("Downloading icon for {0}...", Title));
                var ret = AssetsInfo.Where(info => info.AssetType == XboxAssetType.Icon).Select(info => info.GetAsset()).ToArray();
                return ret;
            }
        }

        public XboxAsset[] BannerAssets {
            get {
                if(AssetsInfo.Where(info => info.AssetType == XboxAssetType.Banner).Any(info => !info.HaveAsset))
                    XboxAssetDownloader.SendStatusChanged(string.Format("Downloading banner for {0}...", Title));
                var ret = AssetsInfo.Where(info => info.AssetType == XboxAssetType.Banner).Select(info => info.GetAsset()).ToArray();
                return ret;
            }
        }

        public XboxAsset[] BackgroundAssets {
            get {
                if(AssetsInfo.Where(info => info.AssetType == XboxAssetType.Background).Any(info => !info.HaveAsset))
                    XboxAssetDownloader.SendStatusChanged(string.Format("Downloading background for {0}...", Title));
                var ret = AssetsInfo.Where(info => info.AssetType == XboxAssetType.Background).Select(info => info.GetAsset()).ToArray();
                return ret;
            }
        }

        public XboxAsset[] ScreenshotsAssets {
            get {
                if(AssetsInfo.Where(info => info.AssetType == XboxAssetType.Screenshot).Any(info => !info.HaveAsset))
                    XboxAssetDownloader.SendStatusChanged(string.Format("Downloading screenshot(s) for {0}...", Title));
                var ret = AssetsInfo.Where(info => info.AssetType == XboxAssetType.Screenshot).Select(info => info.GetAsset()).ToArray();
                return ret;
            }
        }

        private static void ParseXml(Stream xmlData, XboxTitleInfo titleInfo) {
            XboxAssetDownloader.SendStatusChanged("Parsing online asset information, please wait...");
            var ret = new List<XboxAssetInfo>();
            using(var xml = XmlReader.Create(xmlData)) {
                while(xml.Read()) {
                    if(!xml.IsStartElement())
                        continue;
                    var name = xml.Name.ToLower();
                    if(!name.StartsWith("live:"))
                        continue;
                    name = name.Substring(5);
                    if(name == "fulltitle") {
                        xml.Read();
                        titleInfo.Title = xml.Value;
                    }
                    if(name != "image")
                        continue;
                    while(xml.Read() && !(!xml.IsStartElement() && xml.Name.ToLower() == "live:image")) {
                        if(!xml.IsStartElement() || xml.Name.ToLower() != "live:fileurl")
                            continue;
                        xml.Read();
                        var url = new Uri(xml.Value);
                        var fname = Path.GetFileNameWithoutExtension(url.LocalPath);
                        if(fname.StartsWith("banner", StringComparison.CurrentCultureIgnoreCase))
                            ret.Add(new XboxAssetInfo(url, XboxAssetType.Banner, titleInfo));
                        else if(fname.StartsWith("background", StringComparison.CurrentCultureIgnoreCase))
                            ret.Add(new XboxAssetInfo(url, XboxAssetType.Background, titleInfo));
                        else if(fname.StartsWith("tile", StringComparison.CurrentCultureIgnoreCase))
                            ret.Add(new XboxAssetInfo(url, XboxAssetType.Icon, titleInfo));
                        else if(fname.StartsWith("screen", StringComparison.CurrentCultureIgnoreCase))
                            ret.Add(new XboxAssetInfo(url, XboxAssetType.Screenshot, titleInfo));
                        //Ignore anything else
                        break; // We're done with this image
                    }
                }
            }
            titleInfo.AssetsInfo = ret.ToArray();
            XboxAssetDownloader.SendStatusChanged("Finished parsing asset information...");
        }

        private static void ParseHtml(string url, XboxTitleInfo titleInfo)
        {
            XboxAssetDownloader.SendStatusChanged("Fetching online assets, please wait...");
            var ret = new List<XboxAssetInfo>();

            try
            {
                var wc = new WebClient();
                var tmp = new List<string>();
                var data = Encoding.UTF8.GetString(wc.DownloadData(url)).Split('>');
                for (var i = 0; i < data.Length; i++)
                {
                    //game title
                    if (data[i].ToLower().Contains("div id=\"gamedetails\""))
                    {
                        var name = data[i + 1];
                        if (name.ToLower().Contains("<h1"))
                        {
                            name = data[i + 2];
                            titleInfo.Title = name.Substring(0, name.Length - 4);
                        }
                    }

                    //banner image
                    if (data[i].ToLower().Contains("bannerimage.src = "))
                    {
                        var index = data[i].ToLower().IndexOf("http\\x3a", StringComparison.Ordinal);
                        if (index > 0)
                        {
                            var index_end = data[i].ToLower().IndexOf("banner.png';", index, StringComparison.Ordinal);
                            if (index_end > 0)
                            {
                                var name = data[i].Substring(index, index_end - index + 10);
                                name = name.Replace("\\x3a", ":");
                                name = name.Replace("\\x2f","/");
                                var banner_url = new Uri(name);
                                ret.Add(new XboxAssetInfo(banner_url, XboxAssetType.Banner, titleInfo));
                            }
                        }
                    }

                    //background image
                    if (data[i].ToLower().Contains("style=\\\"background-image: url("))
                    {
                        var index = data[i].ToLower().IndexOf("style=\\\"background-image: url(", StringComparison.Ordinal) + 30;
                        if (index > 0)
                        {
                            var index_end = data[i].ToLower().IndexOf("background.jpg", index, StringComparison.Ordinal);
                            if (index_end > 0)
                            {
                                var name = data[i].Substring(index, index_end - index + 14);
                                var backgr_url = new Uri(name);
                                ret.Add(new XboxAssetInfo(backgr_url, XboxAssetType.Background, titleInfo));
                            }
                        }
                    }

                    //screenshots
                    if (data[i].ToLower().Contains("div id=\"mediacontrol\" class=\"grid-18"))
                    {
                        int w = i + 1, n = 0;
                        while (data[w + n].ToLower().Contains("div id=\"image"))
                        {
                            var index = data[w + n + 1].ToLower().IndexOf("http://", StringComparison.Ordinal);
                            if (index > 0)
                            {
                                var index_end = data[w + n + 1].ToLower().IndexOf("screenlg", index, StringComparison.Ordinal);
                                if (index_end > 0)
                                {
                                    index_end = data[w + n + 1].ToLower().IndexOf(".jpg", index_end, StringComparison.Ordinal);
                                    var name = data[w + n + 1].Substring(index, index_end - index + 4);

                                    var screen_url = new Uri(name);
                                    ret.Add(new XboxAssetInfo(screen_url, XboxAssetType.Screenshot, titleInfo));
                                }
                            }
                            n += 3;
                        }
                        i = w + n;
                    }
                    

                }
            }
            catch {  }

            titleInfo.AssetsInfo = ret.ToArray();
            XboxAssetDownloader.SendStatusChanged("Finished parsing asset information...");
        }

        public static XboxTitleInfo FromTitleId(uint titleId, XboxLocale locale) {
            var ret = new XboxTitleInfo {
                                            TitleId = string.Format("{0:X08}", titleId),
                                            Locale = locale.ToString()
                                        };
            var wc = new WebClient();
            var url =
                string.Format(
//                              "http://catalog.xboxlive.com/Catalog/Catalog.asmx/Query?methodName=FindGames&Names=Locale&Values={0}&Names=LegalLocale&Values={0}&Names=Store&Values=1&Names=PageSize&Values=100&Names=PageNum&Values=1&Names=DetailView&Values=5&Names=OfferFilterLevel&Values=1&Names=MediaIds&Values=66acd000-77fe-1000-9115-d802{1:X8}&Names=UserTypes&Values=2&Names=MediaTypes&Values=1&Names=MediaTypes&Values=21&Names=MediaTypes&Values=23&Names=MediaTypes&Values=37&Names=MediaTypes&Values=46",
                                "http://marketplace.xbox.com/{0}/Product/66acd000-77fe-1000-9115-d802{1:X8}",                
                              locale.Locale, titleId);
            XboxAssetDownloader.SendStatusChanged("Fetching online assets, please wait...");
//            using(var stream = new MemoryStream(wc.DownloadData(url)))
//                ParseXml(stream, ret);
                  ParseHtml(url, ret);
            return ret;
        }

        public class XboxAsset {
            public readonly XboxAssetType AssetType;

            public readonly Image Image;

            public XboxAsset(Image image, XboxAssetType assetType) {
                Image = image;
                AssetType = assetType;
            }
        }

        public class XboxAssetInfo {
            public readonly XboxAssetType AssetType;
            public readonly Uri AssetUrl;
            private readonly XboxTitleInfo _titleInfo;
            private XboxAsset _asset;

            public XboxAssetInfo(Uri assetUrl, XboxAssetType assetType, XboxTitleInfo titleInfo) {
                AssetUrl = assetUrl;
                AssetType = assetType;
                _titleInfo = titleInfo;
            }

            public bool HaveAsset { get { return _asset != null; } }

            public XboxAsset GetAsset() {
                if(_asset != null)
                    return _asset; // We already have it
                var wc = new WebClient();
                var data = wc.DownloadData(AssetUrl);
                var ms = new MemoryStream(data);
                var img = Image.FromStream(ms);
                return _asset = new XboxAsset(img, AssetType);
            }

            public override string ToString() { return string.Format("{0} [ {1} ] {2}", _titleInfo.Title, _titleInfo.TitleId, AssetType); }
        }
    }

    public class XboxLocale {
        public readonly string Locale;

        private readonly string _name;

        public XboxLocale(string locale, string name) {
            Locale = locale;
            _name = name;
        }

        public override string ToString() { return string.Format("{0} [ {1} ]", _name, Locale); }
    }

    [DataContract] public class XboxKeywordResponse {
        [DataMember(Name = "entries")] public Entry[] Entries { get; set; }

        [DataContract] public class Entry {
            [DataMember(Name = "detailsUrl")] public string DetailsUrl { get; set; }
            //There is more data sent both here and ^, but we only need this, so i only added that...
        }
    }
}
