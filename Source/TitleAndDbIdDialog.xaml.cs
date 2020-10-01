// 
// 	TitleAndDbIdDialog.xaml.cs
// 	AuroraAssetEditor
// 
// 	Created by Swizzy on 10/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor {
    using System.Globalization;
    using System.Text.RegularExpressions;
    using System.Windows;
    using System.Windows.Controls;
    using System.Windows.Input;

    public partial class TitleAndDbIdDialog {
        public TitleAndDbIdDialog(Window owner) {
            InitializeComponent();
            Icon = App.WpfIcon;
            Owner = owner;
        }

        public string TitleId { get { return TitleIdBox.Text; } }

        public string AssetId { get { return string.Format("{0}_{1}", TitleIdBox.Text, DbIdBox.Text); } }

        private void btnDialogOk_Click(object sender, RoutedEventArgs e) { DialogResult = true; }

        private void OnTextInput(object sender, TextCompositionEventArgs e) {
            uint tmp;
            e.Handled = !uint.TryParse(e.Text, NumberStyles.HexNumber, CultureInfo.CurrentCulture, out tmp);
        }

        private void OnTextChanged(object sender, TextChangedEventArgs e) {
            TitleIdBox.Text = Regex.Replace(TitleIdBox.Text, "[^a-fA-F0-9]+", "");
            DbIdBox.Text = Regex.Replace(DbIdBox.Text, "[^a-fA-F0-9]+", "");
            OkButton.IsEnabled = TitleIdBox.Text.Length == 8 && DbIdBox.Text.Length == 8;
        }
    }
}