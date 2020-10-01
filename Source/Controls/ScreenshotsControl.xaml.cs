//
// 	ScreenshotsControl.xaml.cs
// 	AuroraAssetEditor
//
// 	Created by Swizzy on 10/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Controls {
    using System;
    using System.ComponentModel;
    using System.Drawing.Imaging;
    using System.IO;
    using System.Linq;
    using System.Windows;
    using System.Windows.Controls;
    using System.Windows.Media.Imaging;
    using Classes;
    using Microsoft.Win32;
    using Image = System.Drawing.Image;
    using Size = System.Drawing.Size;

    /// <summary>
    ///     Interaction logic for ScreenshotsControl.xaml
    /// </summary>
    public partial class ScreenshotsControl {
        private readonly MainWindow _main;
        internal bool HavePreview;
        private AuroraAsset.AssetFile _assetFile;
        private Image[] _screenshots;

        public ScreenshotsControl(MainWindow main) {
            InitializeComponent();
            _main = main;
            _assetFile = new AuroraAsset.AssetFile();
            _screenshots = new Image[AuroraAsset.AssetType.ScreenshotEnd - AuroraAsset.AssetType.ScreenshotStart];
            CBox.Items.Clear();
            for(var i = 0; i < _screenshots.Length; i++)
                CBox.Items.Add(new ScreenshotDisplay(i));
            CBox.SelectedIndex = 0;
        }

        public bool HaveScreenshots { get { return _screenshots.Any(t => t != null); } }

        public void Save() {
            var sfd = new SaveFileDialog();
            if(sfd.ShowDialog() != true)
                return;
            Save(sfd.FileName);
        }

        public void Save(string filename) {
            var index = 1;
            foreach(var img in _screenshots.Where(img => img != null)) { // Loop screenshots adding them in the order the user wants them with no interruption
                if(!img.Equals(_assetFile.GetScreenshot(index))) // Don't replace it if it's already where we want it to be
                    _assetFile.SetScreenshot(img, index, _main.UseCompression.IsChecked); // Add screenshot with current settings
                index++; // Increment index so we put next image in next slot
            }
            if(index - 1 < _screenshots.Length) // Do we have any slots that are not used?
            {
                for(; index - 1 < _screenshots.Length; index++) // Loop remaining slots
                    _assetFile.SetScreenshot(null, index, false); // Remove unused slots
            }
            File.WriteAllBytes(filename, _assetFile.FileData);
        }

        public void Reset() {
            SetPreview(null);
            _assetFile = new AuroraAsset.AssetFile();
            _screenshots = new Image[AuroraAsset.AssetType.ScreenshotEnd - AuroraAsset.AssetType.ScreenshotStart];
            CBox.SelectedIndex = 0;
        }

        public void Load(AuroraAsset.AssetFile asset) {
            _assetFile.SetScreenshots(asset);
            Dispatcher.Invoke(new Action(() => {
                                             _screenshots = _assetFile.GetScreenshots();
                                             CBox_SelectionChanged(null, null);
                                         }));
        }

        private void SetPreview(Image img) {
            if(img == null) {
                PreviewImg.Source = new BitmapImage(new Uri("pack://application:,,,/Resources/Placeholders/screenshot.png", UriKind.Absolute));
                HavePreview = false;
                return;
            }
            var bi = new BitmapImage();
            var ms = new MemoryStream();
            img.Save(ms, ImageFormat.Bmp);
            ms.Seek(0, SeekOrigin.Begin);
            bi.BeginInit();
            bi.StreamSource = ms;
            bi.EndInit();
            PreviewImg.Source = bi;
            HavePreview = true;
        }

        public void Load(Image img, bool replace) {
            var index = -1;
            if(replace) {
                var disp = CBox.SelectedItem as ScreenshotDisplay;
                if(disp == null) {
                    for(var i = 0; i < _screenshots.Length; i++) {
                        if(_screenshots[i] != null)
                            continue;
                        index = i;
                        break;
                    }
                }
                else
                    index = disp.Index;
            }
            else {
                for(var i = 0; i < _screenshots.Length; i++) {
                    if(_screenshots[i] != null)
                        continue;
                    index = i;
                    break;
                }
            }
            if(index == -1) {
                MessageBox.Show("There is no space left for new screenshots :(", "No space left", MessageBoxButton.OK, MessageBoxImage.Error);
                return;
            }
            var shouldUseCompression = false;
            Dispatcher.Invoke(new Action(() => shouldUseCompression = _main.UseCompression.IsChecked));
            _assetFile.SetScreenshot(img, index + 1, shouldUseCompression);
            Dispatcher.Invoke(new Action(() => {
                                             _screenshots[index] = img;
                                             SetPreview(img);
                                             CBox.SelectedIndex = index;
                                         }));
        }

        public bool SelectedExists() {
            var disp = CBox.SelectedItem as ScreenshotDisplay;
            if(disp != null)
                return _screenshots[disp.Index] != null; // If true, we want to ask if we should add new one or replace existing
            return false;
        }

        public bool SpaceLeft() { return _screenshots.Any(t => t == null); }

        private void CBox_SelectionChanged(object sender, SelectionChangedEventArgs e) {
            var disp = CBox.SelectedItem as ScreenshotDisplay;
            if(disp == null)
                return;
            SetPreview(_screenshots[disp.Index]);
        }

        private void OnDragEnter(object sender, DragEventArgs e) { _main.OnDragEnter(sender, e); }

        private void OnDrop(object sender, DragEventArgs e) { _main.DragDrop(this, e); }

        internal void RemoveScreenshot(object sender, RoutedEventArgs e) { Load(null, true); }

        internal void SaveImageToFileOnClick(object sender, RoutedEventArgs e) {
            var disp = CBox.SelectedItem as ScreenshotDisplay;
            if(disp == null)
                return;
            MainWindow.SaveToFile(_screenshots[disp.Index], "Choose Location to Save Screenshot Image", string.Format("screenshot{0}.png", disp.Index + 1));
        }

        internal void SelectNewScreenshot(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             var img = _main.LoadImage("Select Screenshot Image(s)", "screenshot.png", new Size(1000, 562));
                             if(img != null)
                                 Load(img, true);
                         };
            bw.RunWorkerCompleted += (o, args) => _main.BusyIndicator.Visibility = Visibility.Collapsed;
            bw.RunWorkerAsync();
        }

        internal void AddNewScreenshot(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             var imglist = _main.LoadImages("Select Screenshot Image(s)", "screenshot.png", new Size(1000, 562));
                             if(imglist == null)
                                 return;
                             foreach(var img in imglist)
                                 Load(img, false);
                         };
            bw.RunWorkerCompleted += (o, args) => _main.BusyIndicator.Visibility = Visibility.Collapsed;
            bw.RunWorkerAsync();
        }

        private void OnContextMenuOpening(object sender, ContextMenuEventArgs e) {
            SaveContextMenuItem.IsEnabled = HavePreview;
            RemoveContextMenuItem.IsEnabled = HavePreview;
        }

        public byte[] GetData() { return _assetFile.FileData; }

        private class ScreenshotDisplay {
            private readonly int _index;

            public ScreenshotDisplay(int index) { _index = index; }

            public int Index { get { return _index; } }

            public override string ToString() { return string.Format("{0}", _index + 1); }
        }
    }
}