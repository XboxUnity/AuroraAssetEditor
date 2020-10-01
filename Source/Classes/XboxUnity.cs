// 
// 	XboxUnity.cs
// 	AuroraAssetEditor
// 
// 	Created by Swizzy on 10/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Classes {
    using System;
    using System.Drawing;
    using System.IO;
    using System.Linq;
    using System.Net;
    using System.Runtime.Serialization;
    using System.Runtime.Serialization.Json;
    using System.Web;

    internal static class XboxUnity {
        private static readonly DataContractJsonSerializer Serializer = new DataContractJsonSerializer(typeof(UnityResponse[]));

        private static string GetUnityUrl(string searchTerm) { return string.Format("http://xboxunity.net/api/Covers/{0}", HttpUtility.UrlEncode(searchTerm)); }

        public static XboxUnityAsset[] GetUnityCoverInfo(string searchTerm) {
            using(var wc = new WebClient()) {
                try {
                    var stream = wc.OpenRead(GetUnityUrl(searchTerm));
                    return stream != null ? ((UnityResponse[])Serializer.ReadObject(stream)).Select(t => new XboxUnityAsset(t)).ToArray() : new XboxUnityAsset[0];
                }
                catch(Exception ex) {
                    MainWindow.SaveError(ex);
                    return new XboxUnityAsset[0];
                }
            }
        }

        public static string GetHomebrewTitleFromFtp(string path) {
            var data = App.FtpOperations.GetAssetData("GameCoverInfo.bin", path);
            if(data == null || data.Length < 10)
                return "N/A";
            using(var ms = new MemoryStream(data)) {
                var response = (UnityResponse[])Serializer.ReadObject(ms);
                return response.Length <= 0 ? "N/A" : response[0].Name;
            }
        }

        [DataContract] internal class UnityResponse {
            [DataMember(Name = "titleid")] public string TitleId { get; set; }

            [DataMember(Name = "name")] public string Name { get; set; }

            [DataMember(Name = "official")] public bool Official { get; set; }

            [DataMember(Name = "filesize")] public string FileSize { get; set; }

            [DataMember(Name = "url")] public string Url { get; set; }

            [DataMember(Name = "front")] public string Front { get; set; }

            [DataMember(Name = "thumbnail")] public string Thumbnail { get; set; }

            [DataMember(Name = "author")] public string Author { get; set; }

            [DataMember(Name = "uploaddate")] public string UploadDate { get; set; }

            [DataMember(Name = "rating")] public string Rating { get; set; }

            [DataMember(Name = "link")] public string Link { get; set; }
        }

        public class XboxUnityAsset {
            private readonly UnityResponse _unityResponse;
            private Image _cover;

            public XboxUnityAsset(UnityResponse response) { _unityResponse = response; }

            public bool HaveAsset { get { return _cover != null; } }

            public string Title { get { return _unityResponse.Name; } }

            private static Image GetImage(string url) {
                var wc = new WebClient();
                var data = wc.DownloadData(url);
                var ms = new MemoryStream(data);
                return Image.FromStream(ms);
            }

            public Image GetCover() {
                if(_cover != null)
                    return _cover;
                return _cover = GetImage(_unityResponse.Url);
            }

            public override string ToString() {
                return string.Format(_unityResponse.Official ? "Official cover for {0} Rating: {1}" : "Cover for {0} Rating: {1}", _unityResponse.Name, _unityResponse.Rating ?? "N/A");
            }
        }
    }
}