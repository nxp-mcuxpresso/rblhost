// Copyright 2026 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
// Read and parse CurrentVersion tag from a device, specified as the first CLI argument.
// An optional second argument selects the protocol: "uart" (default), "i2c", or "usb".
// Mirrors examples/c/get_property.c and examples/c/get_property_usb.c, using the
// csbindgen-generated P/Invoke bindings (see root README, "Building C# (.NET) bindings").
using System;
using System.Text;
using RblHost;

if (args.Length < 1)
{
    Console.WriteLine("usage: get_property <device identifier> [uart|i2c|usb]");
    Console.WriteLine("  uart (default): serial port name, e.g. \"COM1\" or \"/dev/ttyACM0\"");
    Console.WriteLine("  i2c: I2C device identifier, e.g. \"/dev/i2c-1:0x10\"");
    Console.WriteLine("  usb: USB-HID VID/PID (and optional selector), e.g. \"0x1FC9,0x0135\"");
    return 1;
}

string devicePath = args[0];
CProtocol protocol = CProtocol.UART;

if (args.Length >= 2)
{
    protocol = args[1].ToLowerInvariant() switch
    {
        "uart" => CProtocol.UART,
        "i2c" => CProtocol.I2c,
        "usb" => CProtocol.Usb,
        _ => throw new ArgumentException($"unknown protocol \"{args[1]}\", expected uart, i2c, or usb"),
    };
}

unsafe
{
    byte[] devicePathBytes = Encoding.UTF8.GetBytes(devicePath + "\0");

    fixed (byte* pDevicePath = devicePathBytes)
    {
        // mboot_create returns an opaque pointer (void*). Wrapping it in IntPtr lets
        // the rest of the example avoid raw pointers outside of this unsafe block.
        IntPtr device = (IntPtr)NativeMethods.mboot_create(pDevicePath, protocol);
        if (device == IntPtr.Zero)
        {
            Console.WriteLine("device is NULL");
            return 1;
        }

        try
        {
            CGetPropertyResponse response;
            // Sending get_property, tag number 1 is for current version.
            int status = NativeMethods.mboot_get_property((void*)device, 1, 0, &response);

            if (status < 0)
            {
                Console.Error.WriteLine("error occured while running get_property");
                return 1;
            }

            try
            {
                // Reading response words
                for (nuint i = 0; i < response.response_words_len; i++)
                {
                    Console.Error.WriteLine($"word #{i}: 0x{response.response_words[i]:X}");
                }

                // Parsing version data
                if (response.response_words_len > 0)
                {
                    uint versionWord = response.response_words[0];

                    char mark = (char)((versionWord >> 24) & 0xFF);
                    byte major = (byte)((versionWord >> 16) & 0xFF);
                    byte minor = (byte)((versionWord >> 8) & 0xFF);
                    byte fixation = (byte)(versionWord & 0xFF);

                    Console.WriteLine($"Version: {mark}{major}.{minor}.{fixation}");
                }
                else
                {
                    Console.Error.WriteLine("response word length is incorrect");
                    return 1;
                }
            }
            finally
            {
                // Free-ing allocated memory
                NativeMethods.mboot_free_response_words(response.response_words, response.response_words_len);
            }
        }
        finally
        {
            NativeMethods.mboot_destroy((void*)device);
        }
    }
}

return 0;
