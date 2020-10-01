//
// 	IconBannerControl.xaml.cs
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
    ///     Interaction logic for IconBannerControl.xaml
    /// </summary>
    public partial class IconBannerControl {
        private readonly MainWindow _main;
        internal bool HaveBanner;
        internal bool HaveIcon;
        private AuroraAsset.AssetFile _assetFile;

        public IconBannerControl(MainWindow main) {
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
            SetPreview(null, true);
            SetPreview(null, false);
            _assetFile = new AuroraAsset.AssetFile();
        }

        public void Load(AuroraAsset.AssetFile asset) {
            _assetFile.SetIcon(asset);
            _assetFile.SetBanner(asset);
            Dispatcher.Invoke(new Action(() => SetPreview(_assetFile.GetIcon(), true)));
            Dispatcher.Invoke(new Action(() => SetPreview(_assetFile.GetBanner(), false)));
        }

        private void SetPreview(Image img, bool icon) {
            if(img == null) {
                if(icon) {
                    PreviewIcon.Source = new BitmapImage(new Uri("pack://application:,,,/Resources/Placeholders/icon.png", UriKind.Absolute));
                    HaveIcon = false;
                }
                else {
                    PreviewBanner.Source = new BitmapImage(new Uri("pack://application:,,,/Resources/Placeholders/banner.png", UriKind.Absolute));
                    HaveBanner = false;
                }
                return;
            }
            var bi = new BitmapImage();
            bi.BeginInit();
            var ms = new MemoryStream();
            img.Save(ms, ImageFormat.Bmp);
            ms.Seek(0, SeekOrigin.Begin);
            bi.StreamSource = ms;
            bi.EndInit();
            if(icon) {
                PreviewIcon.Source = bi;
                HaveIcon = true;
            }
            else {
                PreviewBanner.Source = bi;
                HaveBanner = true;
            }
        }

        public void Load(Image img, bool icon) {
            var shouldUseCompression = false;
            Dispatcher.Invoke(new Action(() => shouldUseCompression = _main.UseCompression.IsChecked));
            if(icon)
                _assetFile.SetIcon(img, shouldUseCompression);
            else
                _assetFile.SetBanner(img, shouldUseCompression);
            Dispatcher.Invoke(new Action(() => SetPreview(img, icon)));
        }

        private void OnDragEnter(object sender, DragEventArgs e) { _main.OnDragEnter(sender, e); }

        private void OnDrop(object sender, DragEventArgs e) { _main.DragDrop(this, e); }

        internal void SaveIconToFileOnClick(object sender, RoutedEventArgs e) { MainWindow.SaveToFile(_assetFile.GetIcon(), "Choose Location to Save Icon Image", "icon.png"); }

        internal void SaveBannerToFileOnClick(object sender, RoutedEventArgs e) { MainWindow.SaveToFile(_assetFile.GetBanner(), "Choose Location to Save Banner Image", "banner.png"); }

        internal void SelectNewIcon(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             var img = _main.LoadImage("Select Icon Image", "icon.png", new Size(64, 64));
                             if(img != null)
                                 Load(img, true);
                         };
            bw.RunWorkerCompleted += (o, args) => _main.BusyIndicator.Visibility = Visibility.Collapsed;
            bw.RunWorkerAsync();
        }

        internal void SelectNewBanner(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             var img = _main.LoadImage("Select Banner Image", "banner.png", new Size(420, 96));
                             if(img != null)
                                 Load(img, false);
                         };
            bw.RunWorkerCompleted += (o, args) => _main.BusyIndicator.Visibility = Visibility.Collapsed;
            bw.RunWorkerAsync();
        }

        private void OnContextMenuOpening(object sender, ContextMenuEventArgs e) {
            SaveIconContextMenuItem.IsEnabled = HaveIcon;
            SaveBannerContextMenuItem.IsEnabled = HaveBanner;
        }

        public byte[] GetData() { return _assetFile.FileData; }
    }
}