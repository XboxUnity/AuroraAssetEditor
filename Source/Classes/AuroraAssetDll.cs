/// ************************************************************************************
/// ** File:        AuroraAssetDll.cs
/// ** Author:      MaesterRowen (Phoenix) - May, 2015
/// ** Description: Wrapper class for interfacing with the AuroraAsset.dll in C#
/// ************************************************************************************

using System;
using System.Runtime.InteropServices;

namespace PhoenixTools
{
    class AuroraAssetDll
    {
        [DllImport("AuroraAsset.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern int ConvertImageToAsset(IntPtr imageData, int imageDataLen, int imageWidth, int imageHeight, int useCompression,
                                                           IntPtr headerData, out int headerDataLen, IntPtr videoData, out int videoDataLen);

        [DllImport("AuroraAsset.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern int ConvertAssetToImage(IntPtr headerData, int headerDataLen, IntPtr videoData, int videoDataLen, IntPtr imageData, out int imageDataLen,
                                                            out int imageWidth, out int imageHeight );

        [DllImport("AuroraAsset.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern int ConvertDDSToImage(IntPtr ddsData, int ddsDataLen, IntPtr imageData, out int imageDataLen, out int imageWidth, out int imageHeight);

        /// <summary>
        /// Takes raw pixel data in linear ARGB format and outputs Aurora .asset formatted header and video data
        /// </summary>
        /// <param name="pixelData"></param>
        /// <param name="useCompression"></param>
        /// <param name="imageWidth"></param>
        /// <param name="imageHeight"></param>
        /// <param name="headerData"></param>
        /// <param name="videoData"></param>
        /// <returns></returns>
        public static bool ProcessImageToAsset(ref byte[] pixelData, int imageWidth, int imageHeight, bool useCompression, ref byte[] headerData, ref byte[] videoData)
        {
            IntPtr hd = IntPtr.Zero, vd = IntPtr.Zero, pd = IntPtr.Zero;
            try
            {
                bool status = false;

                // Store our pixel data array size for later use
                int pixelDataLen = pixelData.Length;
                if (pixelData == null || pixelDataLen == 0)
                {
                    return false;
                }

                // Copy the pixel data to an unmanaged memory buffer
                pd = Marshal.AllocHGlobal(pixelDataLen);
                Marshal.Copy(pixelData, 0, pd, pixelDataLen);

                // Create variables to hold buffer sizes
                int headerDataLen;
                int videoDataLen;
                int result = ConvertImageToAsset(pd, pixelDataLen, imageWidth, imageHeight, useCompression ? 1 : 0, IntPtr.Zero, out headerDataLen, IntPtr.Zero, out videoDataLen);
                if (result == 1)
                {
                    // Allocate unmanaged memory for asset data
                    hd = Marshal.AllocHGlobal(headerDataLen);
                    vd = Marshal.AllocHGlobal(videoDataLen);

                    // Obtain data
                    result = ConvertImageToAsset(pd, pixelDataLen, imageWidth, imageHeight, useCompression ? 1 : 0, hd, out headerDataLen, vd, out videoDataLen);
                    if (result == 1)
                    {
                        // Copy our header data
                        headerData = new byte[headerDataLen];
                        Marshal.Copy(hd, headerData, 0, headerDataLen);

                        // Copy our video data
                        videoData = new byte[videoDataLen];
                        Marshal.Copy(vd, videoData, 0, videoDataLen);

                        status = true;
                    }
                }
                return status;
            }
            catch( Exception e)
            {
                Console.WriteLine(e.Message);
            }
			finally {
				// Clean up unmanaged file data
                if (pd != IntPtr.Zero)
                    Marshal.FreeHGlobal(pd);
                if (hd != IntPtr.Zero)
                    Marshal.FreeHGlobal(hd);
                if (vd != IntPtr.Zero)
                    Marshal.FreeHGlobal(vd);
			}

            return false;
        }

        /// <summary>
        /// Takes asset header and video data and outputs raw pixel data in BGRA format.
        /// </summary>
        /// <param name="headerData"></param>
        /// <param name="videoData"></param>
        /// <param name="pixelData"></param>
        /// <param name="imageWidth"></param>
        /// <param name="imageHeight"></param>
        /// <returns></returns>
        public static bool ProcessAssetToImage(ref byte[] headerData, ref byte[] videoData, ref byte[] pixelData, out int imageWidth, out int imageHeight )
        {
            IntPtr hd = IntPtr.Zero, vd = IntPtr.Zero, pd = IntPtr.Zero;
            try
            {
                bool status = false;

                int headerDataLen = headerData.Length;
                int videoDataLen = videoData.Length;
                if (headerDataLen == 0 || videoDataLen == 0)
                {
                    imageWidth = 0;
                    imageHeight = 0;
                    return false;
                }

                // Copy the pixel data to an unmanaged memory buffer
                hd = Marshal.AllocHGlobal(headerDataLen);
                Marshal.Copy(headerData, 0, hd, headerDataLen);

                vd = Marshal.AllocHGlobal(videoDataLen);
                Marshal.Copy(videoData, 0, vd, videoDataLen);

                // Create variables to hold buffer sizes
                int imageDataLen;
                int result = ConvertAssetToImage( hd, headerDataLen, vd, videoDataLen, IntPtr.Zero, out imageDataLen, out imageWidth, out imageHeight );
                if (result == 1)
                {
                    // Allocate unmanaged memory for asset data
                    pd = Marshal.AllocHGlobal(imageDataLen);

                    // Obtain data
                    result = ConvertAssetToImage(hd, headerDataLen, vd, videoDataLen, pd, out imageDataLen, out imageWidth, out imageHeight);
                    if (result == 1)
                    {
                        // Copy our pixel data
                        pixelData = new byte[imageDataLen];
                        Marshal.Copy(pd, pixelData, 0, imageDataLen);
                        status = true;
                    }
                }
                return status;
            }
            catch (Exception e)
            {
                Console.WriteLine(e.Message);
            }
            finally
            {
                // Clean up unmanaged file data
                if (pd != IntPtr.Zero)
                    Marshal.FreeHGlobal(pd);
                if (hd != IntPtr.Zero)
                    Marshal.FreeHGlobal(hd);
                if (vd != IntPtr.Zero)
                    Marshal.FreeHGlobal(vd);
            }

            imageWidth = 0;
            imageHeight = 0;
            return false;
        }

        /// <summary>
        /// Takes a DDS image data (with header) and outputs raw pixel data in BGRA format.
        /// </summary>
        /// <param name="ddsData"></param>
        /// <param name="pixelData"></param>
        /// <param name="imageWidth"></param>
        /// <param name="imageHeight"></param>
        /// <returns></returns>
        public static bool ProcessDDSToImage(ref byte[] ddsData, ref byte[] pixelData, out int imageWidth, out int imageHeight)
        {
            IntPtr dds = IntPtr.Zero, pd = IntPtr.Zero;
            try
            {
                bool status = false;

                int ddsDataLen = ddsData.Length;
                if (ddsDataLen == 0)
                {
                    imageWidth = 0;
                    imageHeight = 0;
                    return false;
                }

                // Copy the pixel data to an unmanaged memory buffer
                dds = Marshal.AllocHGlobal(ddsDataLen);
                Marshal.Copy(ddsData, 0, dds, ddsDataLen);

                // Create variables to hold buffer sizes
                int imageDataLen;
                int result = ConvertDDSToImage(dds, ddsDataLen, IntPtr.Zero, out imageDataLen, out imageWidth, out imageHeight);
                if (result == 1)
                {
                    // Allocate unmanaged memory for asset data
                    pd = Marshal.AllocHGlobal(imageDataLen);

                    // Obtain data
                    result = ConvertDDSToImage(dds, ddsDataLen, pd, out imageDataLen, out imageWidth, out imageHeight);
                    if (result == 1)
                    {
                        // Copy our pixel data
                        pixelData = new byte[imageDataLen];
                        Marshal.Copy(pd, pixelData, 0, imageDataLen);
                        status = true;
                    }
                }
                return status;
            }
            catch (Exception e)
            {
                Console.WriteLine(e.Message);
            }
            finally
            {
                // Clean up unmanaged file data
                if (pd != IntPtr.Zero)
                    Marshal.FreeHGlobal(pd);
                if (dds != IntPtr.Zero)
                    Marshal.FreeHGlobal(dds);
            }

            imageWidth = 0;
            imageHeight = 0;
            return false;
        }
    }
}