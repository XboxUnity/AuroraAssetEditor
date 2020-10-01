// 
// 	StatusArgs.cs
// 	AuroraAssetEditor
// 
// 	Created by Swizzy on 10/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Classes {
    using System;

    internal class StatusArgs: EventArgs {
        public readonly string StatusMessage;

        public StatusArgs(string statusMessage) { StatusMessage = statusMessage; }
    }
}