// 
//  BulkActionsDialog.xaml.cs
//  AuroraAssetEditor
// 
//  Created by Cehbab on 23/08/2025
//  Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor {
	using System;
	using System.ComponentModel;
    using System.Globalization;
    using System.Windows;
    using System.Windows.Input;
    using AuroraAssetEditor.Classes;

    public partial class BulkActionsDialog
    {
        private XboxLocale[] _locales;

        public BulkActionsDialog(Window owner) {
            InitializeComponent();
            Icon = App.WpfIcon;
            Owner = owner;

            var bw = new BackgroundWorker();
            bw.DoWork += LocaleWorkerDoWork;
            bw.RunWorkerCompleted += (sender, args) => {
                LocaleBox.ItemsSource = _locales;
                var index = 0;
                for (var i = 0; i < _locales.Length; i++)
                {
                    if (!_locales[i].Locale.Equals("en-us", StringComparison.InvariantCultureIgnoreCase))
                        continue;
                    index = i;
                    break;
                }
                LocaleBox.SelectedIndex = index;
            };
            bw.RunWorkerAsync();

        }

		private void LocaleWorkerDoWork(object sender, DoWorkEventArgs doWorkEventArgs) { _locales = XboxAssetDownloader.GetLocales(); }

		public XboxLocale Locale { get { return LocaleBox.SelectedItem as XboxLocale; } }

		public bool ReplaceExisting { get { return ReplaceExistingChk.IsChecked ?? false; } }

        public bool CoverArtOnly { get { return CoverArtOnlyChk.IsChecked ?? false; } }

        private void btnDialogOk_Click(object sender, RoutedEventArgs e) { DialogResult = true; }

        private void OnTextInput(object sender, TextCompositionEventArgs e) {
            uint tmp;
            e.Handled = !uint.TryParse(e.Text, NumberStyles.HexNumber, CultureInfo.CurrentCulture, out tmp);
        }
    }
}
