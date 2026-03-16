use rust_usb::UsbContext;

fn main() {
    let ctx = UsbContext::new();

    let devices = match ctx.devices() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to enumerate USB devices: {e}");
            std::process::exit(1);
        }
    };

    if devices.is_empty() {
        println!("No WinUSB devices found.");
        println!("Note: only devices with WinUSB.sys installed appear in this list.");
        return;
    }

    println!("Found {} WinUSB device(s):\n", devices.len());

    for (i, info) in devices.iter().enumerate() {
        println!(
            "[{i}] VID:{:04X}  PID:{:04X}  Bus:{:03}  Addr:{:03}",
            info.vendor_id, info.product_id, info.bus_number, info.device_address
        );
        println!("     Path:         {}", info.path);
        println!(
            "     Manufacturer: {}",
            info.manufacturer.as_deref().unwrap_or("<unavailable>")
        );
        println!(
            "     Product:      {}",
            info.product.as_deref().unwrap_or("<unavailable>")
        );
        println!(
            "     Serial:       {}",
            info.serial_number.as_deref().unwrap_or("<unavailable>")
        );

        // Open the device and read the full descriptor tree.
        match ctx.open(&info.path) {
            Ok(handle) => {
                match handle.read_device_descriptor() {
                    Ok(dd) => {
                        println!(
                            "     bcdUSB:       {:#06X}  Class:{:#04X}  SubClass:{:#04X}  Protocol:{:#04X}",
                            dd.bcd_usb, dd.device_class, dd.device_sub_class, dd.device_protocol
                        );
                        println!(
                            "     MaxPkt0:      {}   bcdDevice:{:#06X}  NumConfigs:{}",
                            dd.max_packet_size0, dd.bcd_device, dd.num_configurations
                        );
                    }
                    Err(e) => eprintln!("     [!] read_device_descriptor: {e}"),
                }

                match handle.read_config_descriptor(0) {
                    Ok(cd) => {
                        println!(
                            "     Config[0]:    value={}  interfaces={}  max_power={}mA  self_powered={}",
                            cd.configuration_value,
                            cd.num_interfaces,
                            cd.max_power as u32 * 2,
                            if cd.attributes & 0x40 != 0 { "yes" } else { "no" }
                        );
                        for iface in &cd.interfaces {
                            println!(
                                "       Interface {}: class={:#04X}  subclass={:#04X}  protocol={:#04X}  endpoints={}",
                                iface.interface_number,
                                iface.interface_class,
                                iface.interface_sub_class,
                                iface.interface_protocol,
                                iface.endpoints.len()
                            );
                            for ep in &iface.endpoints {
                                let dir = if ep.endpoint_address & 0x80 != 0 { "IN" } else { "OUT" };
                                let kind = match ep.attributes & 0x03 {
                                    0 => "Control",
                                    1 => "Isochronous",
                                    2 => "Bulk",
                                    _ => "Interrupt",
                                };
                                println!(
                                    "         EP {:#04X} {:3}  {}  MaxPkt={}  Interval={}",
                                    ep.endpoint_address,
                                    dir,
                                    kind,
                                    ep.max_packet_size,
                                    ep.interval
                                );
                            }
                        }
                    }
                    Err(e) => eprintln!("     [!] read_config_descriptor: {e}"),
                }
            }
            Err(e) => eprintln!("     [!] open: {e}"),
        }

        println!();
    }
}
