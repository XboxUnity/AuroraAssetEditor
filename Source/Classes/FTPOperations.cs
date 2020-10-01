// 
// 	FTPOperations.cs
// 	AuroraAssetEditor
// 
// 	Created by Swizzy on 10/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Classes {
    using System;
    using System.Globalization;
    using System.IO;
    using System.Linq;
    using System.Net;
    using System.Net.FtpClient;
    using System.Runtime.Serialization;
    using System.Runtime.Serialization.Json;

    internal class FtpOperations {
        private readonly DataContractJsonSerializer _serializer = new DataContractJsonSerializer(typeof(FtpSettings));
        public EventHandler<StatusArgs> StatusChanged;
        private FtpClient _client;
        private FtpSettings _settings;

        public FtpOperations() { LoadSettings(); }

        public string IpAddress { get { return _settings.IpAddress; } }

        public string Username { get { return _settings.Username; } }

        public string Password { get { return _settings.Password; } }

        public bool HaveSettings { get { return _settings.Loaded; } }

        public bool ConnectionEstablished {
            get {
                if(_client != null && _client.IsConnected)
                    return _client.IsConnected;
                try { MakeConnection(); }
                catch {}
                return _client != null && _client.IsConnected;
            }
        }

        public string Port { get { return _settings.Port; } }

        private void SendStatusChanged(string msg, params object[] param) {
            var handler = StatusChanged;
            if(handler != null)
                handler.Invoke(this, new StatusArgs(string.Format(msg, param)));
        }

        private void LoadSettings() {
            var path = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
            try {
                path = !string.IsNullOrWhiteSpace(path) ? Path.Combine(path, "AuroraAssetEditor", "ftp.json") : "ftp.json";
                using(var stream = File.OpenRead(path))
                    _settings = (FtpSettings)_serializer.ReadObject(stream);
                _settings.Loaded = true;
            }
            catch {
                _settings = new FtpSettings {
                                                Loaded = false
                                            };
            }
        }

        public bool TestConnection(string ip, string user, string pass, string port) {
            _settings.IpAddress = ip;
            _settings.Username = user;
            _settings.Password = pass;
            _settings.Port = port;
            _settings.Loaded = true;
            try {
                if(MakeConnection())
                    return true;
                SendStatusChanged("Connection test Failed...");
                return false;
            }
            catch(Exception ex) {
                SendStatusChanged("Error: {0}", ex.Message);
                return false;
            }
        }

        private bool MakeConnection() {
            _client = new FtpClient {
                                        Credentials = new NetworkCredential(_settings.Username, _settings.Password),
                                        Host = _settings.IpAddress,
                                        SocketKeepAlive = true
                                    };
            int port;
            if(!int.TryParse(_settings.Port, out port)) {
                port = 21;
                _settings.Port = port.ToString(CultureInfo.InvariantCulture);
            }
            _client.Port = port;
            SendStatusChanged("Connecting to {0}...", _settings.IpAddress);
            _client.Connect();
            if(!_client.IsConnected)
                return false;
            SendStatusChanged("Connection to {0} Established checking for Aurora...", _settings.IpAddress);
            var reply = _client.Execute("SITE REVISION");
            if(!reply.Success)
                return false;
            SendStatusChanged("Connection to Aurora revision {0} Established...", reply.Message);
            return true;
        }

        public void SaveSettings() {
            var path = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
            path = !string.IsNullOrWhiteSpace(path) ? Path.Combine(path, "AuroraAssetEditor", "ftp.json") : "ftp.json";
            path = Path.GetFullPath(path);
            if (File.Exists(path))
                File.Delete(path);
            var dir = Path.GetDirectoryName(path);
            if(string.IsNullOrWhiteSpace(dir))
                return;
            if(!Directory.Exists(dir))
                Directory.CreateDirectory(dir);
            using(var stream = File.OpenWrite(path))
                _serializer.WriteObject(stream, _settings);
        }

        public void SaveSettings(string ip, string user, string pass, string port) {
            _settings.IpAddress = ip;
            _settings.Username = user;
            _settings.Password = pass;
            _settings.Port = port;
            _settings.Loaded = true;
            SaveSettings();
        }

        public bool NavigateToGameDataDir() {
            if(_client == null || !_client.IsConnected) {
                if(!MakeConnection()) {
                    SendStatusChanged("Connection failed to {0}", _settings.IpAddress);
                    return false; // Not connected
                }
            }
            if(_client == null)
                return false;
            const string dir = "/Game/Data/GameData/";
            SendStatusChanged("Changing working directory to {0}...", dir);
            _client.SetWorkingDirectory(dir);
            return _client.GetWorkingDirectory().Equals(dir, StringComparison.CurrentCultureIgnoreCase);
        }

        public bool NavigateToAssetDir(string assetName) {
            if(!NavigateToGameDataDir())
                return false;
            var dir = "/Game/Data/GameData/" + assetName + "/";
            SendStatusChanged("Changing working directory to {0}...", dir);
            _client.SetWorkingDirectory(dir);
            return _client.GetWorkingDirectory().Equals(dir, StringComparison.CurrentCultureIgnoreCase);
        }

        public string[] GetDirList() { return (from item in _client.GetListing() where item.Type == FtpFileSystemObjectType.Directory select item.Name).ToArray(); }

        public byte[] GetAssetData(string file, string assetDir) {
            if(!NavigateToAssetDir(assetDir))
                return null;
            var size = (from item in _client.GetListing() where item.Name.Equals(file, StringComparison.CurrentCultureIgnoreCase) select (int)item.Size).FirstOrDefault();
            if(size <= 0)
                return null;
            var data = new byte[size];
            var offset = 0;
            using(var stream = _client.OpenRead(file)) {
                while(offset < data.Length)
                    offset += stream.Read(data, offset, data.Length - offset);
            }
            return data;
        }

        public bool SendAssetData(string file, string assetDir, byte[] data) {
            if(!NavigateToAssetDir(assetDir))
                return false;
            using(var stream = _client.OpenWrite(file))
                stream.Write(data, 0, data.Length);
            return true;
        }

        public bool DownloadContentDb(string path) {
            if(_client == null || !_client.IsConnected) {
                if(!MakeConnection()) {
                    SendStatusChanged("Connection failed to {0}", _settings.IpAddress);
                    return false; // Not connected
                }
            }
            if(_client == null)
                return false;
            const string dir = "/Game/Data/DataBases/";
            SendStatusChanged("Changing working directory to {0}...", dir);
            _client.SetWorkingDirectory(dir);
            if(!_client.GetWorkingDirectory().Equals(dir, StringComparison.CurrentCultureIgnoreCase))
                return false;
            var size = (from item in _client.GetListing() where item.Name.Equals("Content.db", StringComparison.CurrentCultureIgnoreCase) select (int)item.Size).FirstOrDefault();
            if(size <= 0)
                return false;
            var data = new byte[size];
            var offset = 0;
            using(var stream = _client.OpenRead("Content.db")) {
                while(offset < data.Length)
                    offset += stream.Read(data, offset, data.Length - offset);
            }
            File.WriteAllBytes(path, data);
            return true;
        }

        [DataContract] private class FtpSettings {
            public bool Loaded;

            [DataMember(Name = "ip")] public string IpAddress { get; set; }

            [DataMember(Name = "user")] public string Username { get; set; }

            [DataMember(Name = "pass")] public string Password { get; set; }

            [DataMember(Name = "port")] public string Port { get; set; }
        }
    }
}