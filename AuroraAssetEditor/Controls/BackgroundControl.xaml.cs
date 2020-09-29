//
// 	BackgroundControl.xaml.cs
// 	AuroraAssetEditor
//
// 	Created by Swizzy on 10/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Controls {
    using System;
    using System.ComponentModel;
    using System.Drawing.Imaging;
    using System.IO;
    using System.Windows;
    using System.Windows.Controls;
    using System.Windows.Media.Imaging;
    using Classes;
    using Microsoft.Win32;
    using Image = System.Drawing.Image;
    using Size = System.Drawing.Size;

    /// <summary>
    ///     Interaction logic for BackgroundControl.xaml
    /// </summary>
    public partial class BackgroundControl {
        private readonly MainWindow _main;
        internal bool HavePreview;
        private AuroraAsset.AssetFile _assetFile;
        private MemoryStream _memoryStream;

        public BackgroundControl(MainWindow main) {
            InitializeComponent();
            _main = main;
            _assetFile = new AuroraAsset.AssetFile();
        }

        public void Save() {
            var sfd = new SaveFileDialog();
            if(sfd.ShowDialog() == true)
                Save(sfd.FileName);
        }

        public void Save(string filename) { File.WriteAllBytes(filename, _assetFile.FileData); }

        public void Reset() {
            SetPreview(null);
            _assetFile = new AuroraAsset.AssetFile();
        }

        public void Load(AuroraAsset.AssetFile asset) {
            _assetFile.SetBackground(asset);
            Dispatcher.Invoke(new Action(() => SetPreview(_assetFile.GetBackground())));
        }

        private void SetPreview(Image img) {
            if(img == null) {
                PreviewImg.Source = new BitmapImage(new Uri("pack://application:,,,/Resources/Placeholders/background.png", UriKind.Absolute));
                HavePreview = false;
                return;
            }
            if(_memoryStream != null)
                _memoryStream.Close();
            var bi = new BitmapImage();
            _memoryStream = new MemoryStream();
            img.Save(_memoryStream, ImageFormat.Bmp);
            _memoryStream.Seek(0, SeekOrigin.Begin);
            bi.BeginInit();
            bi.StreamSource = _memoryStream;
            bi.EndInit();
            PreviewImg.Source = bi;
            HavePreview = true;
        }

        public void Load(Image img) {
            var shouldUseCompression = false;
            Dispatcher.Invoke(new Action(() => shouldUseCompression = _main.UseCompression.IsChecked));
            _assetFile.SetBackground(img, shouldUseCompression);
            Dispatcher.Invoke(new Action(() => SetPreview(img)));
        }

        private void OnDragEnter(object sender, DragEventArgs e) { _main.OnDragEnter(sender, e); }

        private void OnDrop(object sender, DragEventArgs e) { _main.DragDrop(this, e); }

        internal void SaveImageToFileOnClick(object sender, RoutedEventArgs e) { MainWindow.SaveToFile(_assetFile.GetBackground(), "Choose Location to Save Background Image", "background.png"); }

        internal void SelectNewBackground(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             var img = _main.LoadImage("Select Background Image", "background.png", new Size(1280, 720));
                             if(img != null)
                                 Load(img);
                         };
            bw.RunWorkerCompleted += (o, args) => _main.BusyIndicator.Visibility = Visibility.Collapsed;
            bw.RunWorkerAsync();
        }

        private void OnContextMenuOpening(object sender, ContextMenuEventArgs e) { SaveContextMenuItem.IsEnabled = HavePreview; }

        public byte[] GetData() { return _assetFile.FileData; }
    }
}