// 
// 	ThreadSafeObservableCollection.cs
// 	AuroraAssetEditor
// 
// 	Created by Swizzy on 14/05/2015
// 	Copyright (c) 2015 Swizzy. All rights reserved.

namespace AuroraAssetEditor.Classes {
    using System.Collections.ObjectModel;
    using System.Threading;
    using System.Windows.Threading;

    public class ThreadSafeObservableCollection <T>: ObservableCollection<T> {
        private readonly Dispatcher _dispatcher;
        private readonly ReaderWriterLockSlim _lock;

        public ThreadSafeObservableCollection() {
            _dispatcher = Dispatcher.CurrentDispatcher;
            _lock = new ReaderWriterLockSlim();
        }

        protected override void ClearItems() {
            _dispatcher.InvokeIfRequired(() => {
                                             _lock.EnterWriteLock();
                                             try { base.ClearItems(); }
                                             finally { _lock.ExitWriteLock(); }
                                         }, DispatcherPriority.DataBind);
        }

        protected override void InsertItem(int index, T item) {
            _dispatcher.InvokeIfRequired(() => {
                                             if(index > Count)
                                                 return;

                                             _lock.EnterWriteLock();
                                             try { base.InsertItem(index, item); }
                                             finally { _lock.ExitWriteLock(); }
                                         }, DispatcherPriority.DataBind);
        }

        protected override void MoveItem(int oldIndex, int newIndex) {
            _dispatcher.InvokeIfRequired(() => {
                                             _lock.EnterReadLock();
                                             var itemCount = Count;
                                             _lock.ExitReadLock();

                                             if(oldIndex >= itemCount | newIndex >= itemCount | oldIndex == newIndex)
                                                 return;

                                             _lock.EnterWriteLock();
                                             try { base.MoveItem(oldIndex, newIndex); }
                                             finally { _lock.ExitWriteLock(); }
                                         }, DispatcherPriority.DataBind);
        }

        protected override void RemoveItem(int index) {
            _dispatcher.InvokeIfRequired(() => {
                                             if(index >= Count)
                                                 return;

                                             _lock.EnterWriteLock();
                                             try { base.RemoveItem(index); }
                                             finally { _lock.ExitWriteLock(); }
                                         }, DispatcherPriority.DataBind);
        }

        protected override void SetItem(int index, T item) {
            _dispatcher.InvokeIfRequired(() => {
                                             _lock.EnterWriteLock();
                                             try { base.SetItem(index, item); }
                                             finally { _lock.ExitWriteLock(); }
                                         }, DispatcherPriority.DataBind);
        }
    }
}