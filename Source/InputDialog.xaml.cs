// 
// 	InputDialog.xaml.cs
// 	AuroraAssetEditor
// 
// 	Created by Swizzy on 07/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor {
    using System;
    using System.Windows;

    public partial class InputDialog {
        public InputDialog(Window owner, string information, string defaultValue = "") {
            InitializeComponent();
            Icon = App.WpfIcon;
            Owner = owner;
            InfoLabel.Text = information;
            ValueBox.Text = defaultValue;
        }

        public string Value { get { return ValueBox.Text; } }

        private void btnDialogOk_Click(object sender, RoutedEventArgs e) { DialogResult = true; }

        private void Window_ContentRendered(object sender, EventArgs e) {
            ValueBox.SelectAll();
            ValueBox.Focus();
        }
    }
}