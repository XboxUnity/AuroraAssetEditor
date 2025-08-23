//
//  FtpAssetsControl.xaml.cs
//  AuroraAssetEditor
//
//  Created by Swizzy on 13/05/2015
//  Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Controls {
    using System;
	using System.Collections.Generic;
	using System.ComponentModel;
	using System.Globalization;
	using System.IO;
	using System.Linq;
    using System.Net.NetworkInformation;
    using System.Net.Sockets;
    using System.Threading;
    using System.Windows;
    using System.Windows.Controls;
    using System.Windows.Data;
	using System.Windows.Threading;
    using AuroraAssetEditor.Models;
    using Classes;
    using Helpers;
	using static AuroraAssetEditor.Classes.XboxTitleInfo;

	/// <summary>
	///     Interaction logic for FtpAssetsControl.xaml
	/// </summary>
    public partial class FtpAssetsControl {
		private readonly InternetArchiveDownloader _internetArchiveDownloader = new InternetArchiveDownloader();
		private readonly XboxAssetDownloader _xboxAssetDownloader = new XboxAssetDownloader();
		private readonly Random _rand = new Random();
		private readonly ThreadSafeObservableCollection<AuroraDbManager.ContentItem> _assetsList = new ThreadSafeObservableCollection<AuroraDbManager.ContentItem>();
        private readonly CollectionViewSource _assetsViewSource = new CollectionViewSource();
        private readonly ICollectionView _assetView;
        private readonly BackgroundControl _background;
        private readonly BoxartControl _boxart;
        private readonly IconBannerControl _iconBanner;
        private readonly MainWindow _main;
        private readonly ScreenshotsControl _screenshots;
        private byte[] _buffer;
        private bool _isBusy, _isError;

        public FtpAssetsControl(MainWindow main, BoxartControl boxart, BackgroundControl background, IconBannerControl iconBanner, ScreenshotsControl screenshots) {
            InitializeComponent();
            _assetsViewSource.Source = _assetsList;
            _main = main;
            _boxart = boxart;
            _background = background;
            _iconBanner = iconBanner;
            _screenshots = screenshots;
            App.FtpOperations.StatusChanged += (sender, args) => Dispatcher.Invoke(new Action(() => Status.Text = args.StatusMessage));
            FtpAssetsBox.ItemsSource = _assetView = _assetsViewSource.View;
            if(!App.FtpOperations.HaveSettings) {
                var ip = GetActiveIp();
                var index = ip.LastIndexOf('.');
                if(ip.Length > 0 && index > 0)
                    IpBox.Text = ip.Substring(0, index + 1);
            }
            else {
                IpBox.Text = App.FtpOperations.IpAddress;
                UserBox.Text = App.FtpOperations.Username;
                PassBox.Text = App.FtpOperations.Password;
                PortBox.Text = App.FtpOperations.Port;
            }
        }

        private static string GetActiveIp() {
            foreach(var unicastAddress in
                NetworkInterface.GetAllNetworkInterfaces().Where(f => f.OperationalStatus == OperationalStatus.Up).Select(f => f.GetIPProperties()).Where(
                                                                                                                                                          ipInterface =>
                                                                                                                                                          ipInterface.GatewayAddresses.Count > 0)
                                .SelectMany(
                                            ipInterface =>
                                            ipInterface.UnicastAddresses.Where(
                                                                               unicastAddress =>
                                                                               (unicastAddress.Address.AddressFamily == AddressFamily.InterNetwork) && (unicastAddress.IPv4Mask.ToString() != "0.0.0.0")))
                )
                return unicastAddress.Address.ToString();
            return "";
        }

        private void TestConnectionClick(object sender, RoutedEventArgs e) {
            var ip = IpBox.Text;
            var user = UserBox.Text;
            var pass = PassBox.Text;
            var port = PortBox.Text;
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => App.FtpOperations.TestConnection(ip, user, pass, port);
            bw.RunWorkerCompleted += (o, args) => _main.BusyIndicator.Visibility = Visibility.Collapsed;
            _main.BusyIndicator.Visibility = Visibility.Visible;
            Status.Text = string.Format("Running a connection test to {0}", IpBox.Text);
            bw.RunWorkerAsync();
        }

        private void SaveSettingsClick(object sender, RoutedEventArgs e) { App.FtpOperations.SaveSettings(IpBox.Text, UserBox.Text, PassBox.Text, PortBox.Text); }

        private void FtpAssetsBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if (FtpAssetsBox.SelectedItem is Classes.AuroraDbManager.ContentItem selectedAsset)
            {
                var newGame = new Game
                {
                    Title = selectedAsset.TitleName,
                    TitleId = selectedAsset.TitleId,
                    DbId = selectedAsset.DatabaseId,
                    IsGameSelected = true
                };

                GlobalState.CurrentGame = newGame;
            }
        }

        private void GetAssetsClick(object sender, RoutedEventArgs e) {
            _assetsList.Clear();
            if(!App.FtpOperations.HaveSettings)
                return;
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             try {
                                 var path = Path.Combine(Path.GetTempPath(), "AuroraAssetEditor.db");
                                 if(!App.FtpOperations.DownloadContentDb(path))
                                     return;
                                 foreach(var title in AuroraDbManager.GetDbTitles(path))
                                     _assetsList.Add(title);
                                 args.Result = true;
                             }
                             catch(Exception ex) {
                                 MainWindow.SaveError(ex);
                                 args.Result = false;
                             }
                         };
            bw.RunWorkerCompleted += (o, args) => {
                                         _main.BusyIndicator.Visibility = Visibility.Collapsed;
                                         if((bool)args.Result)
                                             Status.Text = "Finished grabbing FTP Assets information successfully...";
                                         else
                                             Status.Text = "There was an error, check error.log for more information...";
                                     };
            _main.BusyIndicator.Visibility = Visibility.Visible;
            Status.Text = "Grabbing FTP Assets information...";
            bw.RunWorkerAsync();
        }

        private void ProcessAsset(Task task, bool shouldHideWhenDone = true) {
            _isError = false;
            AuroraDbManager.ContentItem asset = null;
            Dispatcher.InvokeIfRequired(() => asset = FtpAssetsBox.SelectedItem as AuroraDbManager.ContentItem, DispatcherPriority.Normal);
            if(asset == null)
                return;
            var bw = new BackgroundWorker();
            bw.DoWork += (sender, args) => {
                             try {
                                 switch(task) {
                                     case Task.GetBoxart:
                                         _buffer = asset.GetBoxart();
                                         break;
                                     case Task.GetBackground:
                                         _buffer = asset.GetBackground();
                                         break;
                                     case Task.GetIconBanner:
                                         _buffer = asset.GetIconBanner();
                                         break;
                                     case Task.GetScreenshots:
                                         _buffer = asset.GetScreenshots();
                                         break;
                                     case Task.SetBoxart:
                                         asset.SaveAsBoxart(_buffer);
                                         break;
                                     case Task.SetBackground:
                                         asset.SaveAsBackground(_buffer);
                                         break;
                                     case Task.SetIconBanner:
                                         asset.SaveAsIconBanner(_buffer);
                                         break;
                                     case Task.SetScreenshots:
                                         asset.SaveAsScreenshots(_buffer);
                                         break;
                                 }
                                 args.Result = true;
                             }
                             catch(Exception ex) {
                                 MainWindow.SaveError(ex);
                                 args.Result = false;
                             }
                         };
            bw.RunWorkerCompleted += (sender, args) => {
                                         if(shouldHideWhenDone)
                                             Dispatcher.InvokeIfRequired(() => _main.BusyIndicator.Visibility = Visibility.Collapsed, DispatcherPriority.Normal);
                                         var isGet = true;
                                         if((bool)args.Result) {
                                             if(_buffer.Length > 0) {
                                                 var aurora = new AuroraAsset.AssetFile(_buffer);
                                                 switch(task) {
                                                     case Task.GetBoxart:
                                                         _boxart.Load(aurora);
                                                         Dispatcher.InvokeIfRequired(() => _main.BoxartTab.IsSelected = true, DispatcherPriority.Normal);
                                                         break;
                                                     case Task.GetBackground:
                                                         _background.Load(aurora);
                                                         Dispatcher.InvokeIfRequired(() => _main.BackgroundTab.IsSelected = true, DispatcherPriority.Normal);
                                                         break;
                                                     case Task.GetIconBanner:
                                                         _iconBanner.Load(aurora);
                                                         Dispatcher.InvokeIfRequired(() => _main.IconBannerTab.IsSelected = true, DispatcherPriority.Normal);
                                                         break;
                                                     case Task.GetScreenshots:
                                                         _screenshots.Load(aurora);
                                                         Dispatcher.InvokeIfRequired(() => _main.ScreenshotsTab.IsSelected = true, DispatcherPriority.Normal);
                                                         break;
                                                     default:
                                                         isGet = false;
                                                         break;
                                                 }
                                             }
                                             if(shouldHideWhenDone && isGet)
                                                 Dispatcher.InvokeIfRequired(() => Status.Text = "Finished grabbing assets from FTP", DispatcherPriority.Normal);
                                             else if(shouldHideWhenDone)
                                                 Dispatcher.InvokeIfRequired(() => Status.Text = "Finished saving assets to FTP", DispatcherPriority.Normal);
                                         }
                                         else {
                                             switch(task) {
                                                 case Task.GetBoxart:
                                                 case Task.GetBackground:
                                                 case Task.GetIconBanner:
                                                 case Task.GetScreenshots:
                                                     break;
                                                 default:
                                                     isGet = false;
                                                     break;
                                             }
                                             if(isGet)
                                                 Dispatcher.InvokeIfRequired(() => Status.Text = "Failed getting asset data... See error.log for more information...", DispatcherPriority.Normal);
                                             else
                                                 Dispatcher.InvokeIfRequired(() => Status.Text = "Failed saving asset data... See error.log for more information...", DispatcherPriority.Normal);
                                             _isError = true;
                                         }
                                         _isBusy = false;
                                     };
            Dispatcher.InvokeIfRequired(() => _main.BusyIndicator.Visibility = Visibility.Visible, DispatcherPriority.Normal);
            _isBusy = true;
            bw.RunWorkerAsync();
        }

		private void GuessClick(Object sender, RoutedEventArgs e)
		{

		}

		private void GetBoxartClick(object sender, RoutedEventArgs e) { ProcessAsset(Task.GetBoxart); }

        private void GetBackgroundClick(object sender, RoutedEventArgs e) { ProcessAsset(Task.GetBackground); }

        private void GetIconBannerClick(object sender, RoutedEventArgs e) { ProcessAsset(Task.GetIconBanner); }

        private void GetScreenshotsClick(object sender, RoutedEventArgs e) { ProcessAsset(Task.GetScreenshots); }

        private void GetFtpAssetsClick(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             ProcessAsset(Task.GetBoxart, false);
                             while(_isBusy)
                                 Thread.Sleep(100);
                             if(_isError)
                                 return;
                             ProcessAsset(Task.GetBackground, false);
                             while(_isBusy)
                                 Thread.Sleep(100);
                             if(_isError)
                                 return;
                             ProcessAsset(Task.GetIconBanner, false);
                             while(_isBusy)
                                 Thread.Sleep(100);
                             if(_isError)
                                 return;
                             ProcessAsset(Task.GetScreenshots);
                         };
            bw.RunWorkerCompleted += (o, args) => _main.BusyIndicator.Visibility = Visibility.Collapsed;
            bw.RunWorkerAsync();
        }

        private void SaveFtpAssetsClick(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             Dispatcher.InvokeIfRequired(() => _buffer = _boxart.GetData(), DispatcherPriority.Normal);
                             ProcessAsset(Task.SetBoxart, false);
                             while(_isBusy)
                                 Thread.Sleep(100);
                             if(_isError)
                                 return;
                             Dispatcher.InvokeIfRequired(() => _buffer = _background.GetData(), DispatcherPriority.Normal);
                             ProcessAsset(Task.SetBackground, false);
                             while(_isBusy)
                                 Thread.Sleep(100);
                             if(_isError)
                                 return;
                             Dispatcher.InvokeIfRequired(() => _buffer = _iconBanner.GetData(), DispatcherPriority.Normal);
                             ProcessAsset(Task.SetIconBanner, false);
                             while(_isBusy)
                                 Thread.Sleep(100);
                             if(_isError)
                                 return;
                             Dispatcher.InvokeIfRequired(() => _buffer = _screenshots.GetData(), DispatcherPriority.Normal);
                             ProcessAsset(Task.SetScreenshots);
                         };
            bw.RunWorkerCompleted += (o, args) => _main.BusyIndicator.Visibility = Visibility.Collapsed;
            _main.BusyIndicator.Visibility = Visibility.Visible;
            bw.RunWorkerAsync();
        }

        private void SaveBoxartClick(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             Dispatcher.InvokeIfRequired(() => _buffer = _boxart.GetData(), DispatcherPriority.Normal);
                             ProcessAsset(Task.SetBoxart);
                         };
            _main.BusyIndicator.Visibility = Visibility.Visible;
            bw.RunWorkerAsync();
        }

        private void SaveBackgroundClick(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             Dispatcher.InvokeIfRequired(() => _buffer = _background.GetData(), DispatcherPriority.Normal);
                             ProcessAsset(Task.SetBackground);
                         };
            _main.BusyIndicator.Visibility = Visibility.Visible;
            bw.RunWorkerAsync();
        }

        private void SaveIconBannerClick(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             Dispatcher.InvokeIfRequired(() => _buffer = _iconBanner.GetData(), DispatcherPriority.Normal);
                             ProcessAsset(Task.SetIconBanner);
                         };
            _main.BusyIndicator.Visibility = Visibility.Visible;
            bw.RunWorkerAsync();
        }

        private void SaveScreenshotsClick(object sender, RoutedEventArgs e) {
            var bw = new BackgroundWorker();
            bw.DoWork += (o, args) => {
                             Dispatcher.InvokeIfRequired(() => _buffer = _screenshots.GetData(), DispatcherPriority.Normal);
                             ProcessAsset(Task.SetScreenshots);
                         };
            _main.BusyIndicator.Visibility = Visibility.Visible;
            bw.RunWorkerAsync();
        }

        private void OnDragEnter(object sender, DragEventArgs e) { _main.OnDragEnter(sender, e); }

        private void OnDrop(object sender, DragEventArgs e) { _main.DragDrop(this, e); }

        private void FtpAssetsBoxContextOpening(object sender, ContextMenuEventArgs e) {
            if(FtpAssetsBox.SelectedItem == null)
                e.Handled = true;
        }

        private void RemoveFtpAssetsClick(object sender, RoutedEventArgs e) {
            _boxart.Reset();
            _iconBanner.Reset();
            _background.Reset();
            _screenshots.Reset();
            SaveFtpAssetsClick(sender, e);
        }

        private void RemoveBoxartClick(object sender, RoutedEventArgs e) {
            _boxart.Reset();
            SaveBoxartClick(sender, e);
        }

        private void RemoveBackgroundClick(object sender, RoutedEventArgs e) {
            _background.Reset();
            SaveBackgroundClick(sender, e);
        }

        private void RemoveIconBannerClick(object sender, RoutedEventArgs e) {
            _iconBanner.Reset();
            SaveIconBannerClick(sender, e);
        }

        private void RemoveScreenshotsClick(object sender, RoutedEventArgs e) {
            _screenshots.Reset();
            SaveScreenshotsClick(sender, e);
        }

        private void TitleFilterChanged(Object sender, TextChangedEventArgs e) => FiltersChanged(TitleFilterBox.Text, TitleIdFilterBox.Text);

        private void TitleIdFilterChanged(Object sender, TextChangedEventArgs e) => FiltersChanged(TitleFilterBox.Text, TitleIdFilterBox.Text);

        private void FiltersChanged(string titleFilter, string titleIdFilter)
        {
            _assetView.Filter = item =>
            {
                var contentItem = item as AuroraDbManager.ContentItem;
                if (contentItem == null)
                    return false;
                if (string.IsNullOrWhiteSpace(titleFilter) && string.IsNullOrWhiteSpace(titleIdFilter))
                    return true;
                if (!string.IsNullOrWhiteSpace(titleFilter) && !string.IsNullOrWhiteSpace(titleIdFilter))
                    return contentItem.TitleName.ToLower().Contains(titleFilter.ToLower()) && contentItem.TitleId.ToLower().Contains(titleIdFilter.ToLower());
                if (!string.IsNullOrWhiteSpace(titleFilter))
                    return contentItem.TitleName.ToLower().Contains(titleFilter.ToLower());
                return contentItem.TitleId.ToLower().Contains(titleIdFilter.ToLower());
            };
        }

		private void SetStatusText(string StatusText)
		{
			Dispatcher.Invoke(new Action(() => Status.Text = StatusText));
		}

		private bool VerifyAsset(AuroraDbManager.ContentItem asset, AuroraAsset.AssetType type)
		{
			byte[] buffer;
			switch (type)
			{
				case AuroraAsset.AssetType.Boxart:
					buffer = asset.GetBoxart();
					break;
				case AuroraAsset.AssetType.Icon:
				case AuroraAsset.AssetType.Banner:
					buffer = asset.GetIconBanner();
					break;
				case AuroraAsset.AssetType.Background:
					buffer = asset.GetBackground();
					break;
				default:
					buffer = asset.GetScreenshots();
					break;
			}
			AuroraAsset.AssetFile aurora = new AuroraAsset.AssetFile(buffer);
			switch (type)
			{
				case AuroraAsset.AssetType.Boxart:
					return (aurora.GetBoxart() != null);
				case AuroraAsset.AssetType.Icon:
					return (aurora.GetIcon() != null);
				case AuroraAsset.AssetType.Banner:
					return (aurora.GetBanner() != null);
				case AuroraAsset.AssetType.Background:
					return (aurora.GetBackground() != null);
				default:
					return (aurora.GetScreenshots().Length != 0);
			}
		}

		private void BulkDownloadClick(Object sender, RoutedEventArgs e)
		{
			var assets = FtpAssetsBox.Items;
			if (assets.Count == 0)
			{
				MessageBox.Show("ERROR: No Assets listed", "ERROR", MessageBoxButton.OK, MessageBoxImage.Error);
				return;
			}
			if (!App.FtpOperations.ConnectionEstablished)
			{
				MessageBox.Show("ERROR: FTP Connection could not be established", "ERROR", MessageBoxButton.OK, MessageBoxImage.Error);
				return;
			}
			var dialog = new BulkActionsDialog(_main);
			if (dialog.ShowDialog() != true)
				return;
			bool replaceExisting = dialog.ReplaceExisting;
			bool coverArtOnly = dialog.CoverArtOnly;
			var shouldUseCompression = false;
			Dispatcher.Invoke(new Action(() => shouldUseCompression = _main.UseCompression.IsChecked));

			var bw = new BackgroundWorker();
			bw.DoWork += (o, args) => {
				int max_ss = 3;
				try
				{
					// forced locale for now :(
					XboxLocale xboxLocale = XboxAssetDownloader.GetLocales().Where(obj => obj.Locale.Equals("en-US")).First();
					foreach (AuroraDbManager.ContentItem asset in FtpAssetsBox.Items)
					{
						bool boxart = false, background = false, iconBanner = false, screenshots = false;
						if (!replaceExisting)
						{
							// do we have content already ?
							if (!App.FtpOperations.ConnectionEstablished)
							{
								MessageBox.Show("ERROR: FTP Connection could not be established", "ERROR", MessageBoxButton.OK, MessageBoxImage.Error);
								return;
							}

							boxart = VerifyAsset(asset, AuroraAsset.AssetType.Boxart);
							if (!coverArtOnly)
							{
								background = VerifyAsset(asset, AuroraAsset.AssetType.Background);
								iconBanner = (VerifyAsset(asset, AuroraAsset.AssetType.Icon) && VerifyAsset(asset, AuroraAsset.AssetType.Banner));
								boxart = VerifyAsset(asset, AuroraAsset.AssetType.Boxart);
								screenshots = VerifyAsset(asset, AuroraAsset.AssetType.ScreenshotStart);
							}
						}

						uint titleId;
						uint.TryParse(asset.TitleId, NumberStyles.HexNumber, CultureInfo.CurrentCulture, out titleId);

						if (!boxart)
						{
							SetStatusText(string.Format("Downloading XBOX Assets for {0}", asset.TitleName));
							InternetArchiveAsset[] archiveResults = _internetArchiveDownloader.GetTitleInfo(titleId);
							if (archiveResults.Length != 0)
							{
								// choose a random result
								int index = _rand.Next(0, archiveResults.Length);
								InternetArchiveAsset archive = archiveResults[index];

								AuroraAsset.AssetFile aurora = new AuroraAsset.AssetFile();
								aurora.SetBoxart(archive.GetCover(), shouldUseCompression);
								if (aurora.HasBoxArt)
								{
									asset.SaveAsBoxart(aurora.FileData);
									boxart = true;
								}
							}
						}

						if (!coverArtOnly && !(background && iconBanner && screenshots))
						{
							int current_ss = 0;
							List<XboxAssetInfo> screenshots_list = new List<XboxAssetInfo>();

							SetStatusText(string.Format("Downloading XBOX Assets for {0}", asset.TitleName));
							XboxTitleInfo xboxResult = _xboxAssetDownloader.GetTitleInfo(titleId, xboxLocale)[0];
							foreach (XboxAssetInfo info in xboxResult.AssetsInfo)
							{
								switch (info.AssetType)
								{
									case XboxTitleInfo.XboxAssetType.Icon:
									case XboxTitleInfo.XboxAssetType.Banner:
										if (!iconBanner)
										{
											AuroraAsset.AssetFile aurora = new AuroraAsset.AssetFile();
											if (info.AssetType == XboxTitleInfo.XboxAssetType.Icon)
												aurora.SetIcon(info.GetAsset().Image, shouldUseCompression);
											else
												aurora.SetBanner(info.GetAsset().Image, shouldUseCompression);

											if (aurora.HasIconBanner)
											{
												asset.SaveAsIconBanner(aurora.FileData);
												iconBanner = true;
											}
										}
										break;
									case XboxTitleInfo.XboxAssetType.Background:
										if (!background)
										{
											AuroraAsset.AssetFile aurora = new AuroraAsset.AssetFile();
											aurora.SetBackground(info.GetAsset().Image, shouldUseCompression);
											if (aurora.HasBackground)
											{
												asset.SaveAsBackground(aurora.FileData);
												background = true;
											}
										}
										break;
									case XboxTitleInfo.XboxAssetType.Screenshot:
										if (!screenshots && current_ss < max_ss)
										{
											current_ss++;

											// save screenshot to list
											screenshots_list.Add(info);
										}
										break;
								}
							}

							if (screenshots_list.Count > 0)
							{
								AuroraAsset.AssetFile aurora = new AuroraAsset.AssetFile();
								int num = 0;
								foreach (XboxAssetInfo info in screenshots_list)
									aurora.SetScreenshot(info.GetAsset().Image, ++num, shouldUseCompression);

								if (aurora.HasScreenshots)
									asset.SaveAsScreenshots(aurora.FileData);
							}
						}
					}
					args.Result = true;
				}
				catch (Exception ex)
				{
					MainWindow.SaveError(ex);
					args.Result = false;
				}
			};
			bw.RunWorkerCompleted += (o, args) => {
				_main.BusyIndicator.Visibility = Visibility.Collapsed;
				if ((bool)args.Result)
					Status.Text = "Finished updating Assets information successfully...";
				else
					Status.Text = "There was an error, check error.log for more information...";
			};
			_main.BusyIndicator.Visibility = Visibility.Visible;
			Status.Text = "Updating Assets information...";
			bw.RunWorkerAsync();
		}

		private enum Task {
            GetBoxart,
            GetBackground,
            GetIconBanner,
            GetScreenshots,
            SetBoxart,
            SetBackground,
            SetIconBanner,
            SetScreenshots,
        }
    }
}
