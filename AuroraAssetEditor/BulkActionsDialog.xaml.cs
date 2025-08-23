// 
//  BulkActionsDialog.xaml.cs
//  AuroraAssetEditor
// 
//  Created by Cehbab on 23/08/2025
//  Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor {
    using System.Globalization;
    using System.Windows;
    using System.Windows.Input;

    public partial class BulkActionsDialog
	{
        public BulkActionsDialog(Window owner) {
            InitializeComponent();
            Icon = App.WpfIcon;
            Owner = owner;
        }

		public bool ReplaceExisting { get { return ReplaceExistingChk.IsChecked ?? false; } }

		public bool CoverArtOnly { get { return CoverArtOnlyChk.IsChecked ?? false; } }

		private void btnDialogOk_Click(object sender, RoutedEventArgs e) { DialogResult = true; }

        private void OnTextInput(object sender, TextCompositionEventArgs e) {
            uint tmp;
            e.Handled = !uint.TryParse(e.Text, NumberStyles.HexNumber, CultureInfo.CurrentCulture, out tmp);
        }
	}
}
