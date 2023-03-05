// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

use log::{debug};

use btle_plat::PeripheralId;
use btleplug::platform as btle_plat;

use btleplug::api::bleuuid::BleUuid;
use btleplug::api::{
    Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter,
    ValueNotification, WriteType,
};
use btleplug::Error as BlePlugError;
use tokio_stream::{StreamExt, StreamMap};

use btle_plat::{Manager, Peripheral};
use std::error::Error;

mod device;
mod state;
mod streams;

use device::Packet;

pub use state::*;
pub use streams::*;

async fn handle_btle_notification(
    central: &btle_plat::Adapter,
    id: &PeripheralId,
    value: &ValueNotification,
    // _tx: AsyncSender,
) -> Result<PeripheralId, BlePlugError> {
    println!("Received notification from {id:?}: {value:?}");
    change_power(central, &id, false).await?;
    Ok(id.clone())
}

async fn change_power(central: &btle_plat::Adapter, peripheral_id: &PeripheralId, power: bool) -> Result<(), BlePlugError> {
    let peripheral = central.peripheral(peripheral_id).await?;
    let pkt = device::Power::from(power);
    let msg = pkt.bytes();
    if power {
        println!("Switching on peripheral {peripheral_id:?}");
    } else {
        println!("Switching off peripheral {peripheral_id:?}");
    }
    peripheral
        .write(&device::DEV_CTL, msg, WriteType::WithoutResponse)
        .await?;
    peripheral
        .write(
            &device::DEV_CTL,
            &device::POWER_STATUS,
            WriteType::WithoutResponse,
        )
        .await?;
    peripheral
        .write(
            &device::DEV_CTL,
            &device::CHANNEL_STATUS,
            WriteType::WithoutResponse,
        )
        .await?;
    Ok(())
}

async fn handle_btle_event(
    event: &btleplug::api::CentralEvent,
    central: &btle_plat::Adapter,
    // peripherals: &mut im::HashMap<PeripheralId, btle_plat::Peripheral>,
    // disconnected: &mut im::HashSet<PeripheralId>,
    stream_map: &mut StreamMap<StreamKey, EventStreams>,
    // tx: AsyncSender,
) -> Result<(), BlePlugError> {
    match event {
        CentralEvent::DeviceDiscovered(id) => {
            debug!("DeviceDiscovered: {:?}", id);
            let peripheral: Peripheral = central.peripheral(id).await?;
            let properties = peripheral.properties().await?;
            if let Some(properties) = properties {
                if let Some(device_name) = properties.local_name {
                    // println!("DeviceDiscovered {id:?}: {device_name:?}");

                    if device_name.contains("NEEWER") {
                        peripheral.connect().await?;
                        println!("connecting to device: {:?}", device_name);
                    }
                }
            };
            // Ok(id.clone())
        }
        CentralEvent::DeviceConnected(id) => {
            println!("DeviceConnected: {:?}", id);
            // central.stop_scan().await?;

            let peripheral = central.peripheral(id).await?;
            let mut retry_count = 0;
            let mut characteristics: Option<
                std::collections::BTreeSet<btleplug::api::Characteristic>,
            > = None;
            while retry_count < 255 && characteristics.is_none() {
                peripheral
                    .discover_services()
                    .await
                    .expect("Discovering services");
                let ctrstc = peripheral.characteristics();
                let has_ctrstc =
                    ctrstc.contains(&device::GATT) && ctrstc.contains(&device::DEV_CTL);
                if has_ctrstc {
                    characteristics = Some(ctrstc);
                } else {
                    retry_count += 1
                }
            }
            println!("Retried finding characteristics: {} times", retry_count);
            if let Some(_characteristics) = characteristics {
                let notifs = peripheral.notifications().await?;
                stream_map.insert(
                    StreamKey::BtleNotifications(id.clone()),
                    EventStreams::BtleNotifications(notifs),
                );

                peripheral.subscribe(&device::GATT).await?;

                peripheral
                    .write(
                        &device::DEV_CTL,
                        &device::POWER_STATUS,
                        WriteType::WithoutResponse,
                    )
                    .await?;
                peripheral
                    .write(
                        &device::DEV_CTL,
                        &device::CHANNEL_STATUS,
                        // WriteType::WithResponse,
                        WriteType::WithoutResponse,
                    )
                    .await?;
            } else {
                peripheral.disconnect().await?;
            }
        }
        CentralEvent::DeviceDisconnected(id) => {
            println!("DeviceDisconnected: {:?}", id);
        }
        CentralEvent::ManufacturerDataAdvertisement {
            id,
            manufacturer_data,
        } => {
            debug!(
                "ManufacturerDataAdvertisement: {:?}, {:?}",
                id, manufacturer_data
            );
        }
        CentralEvent::ServiceDataAdvertisement { id, service_data } => {
            debug!("ServiceDataAdvertisement: {:?}, {:?}", id, service_data);
        }
        CentralEvent::ServicesAdvertisement { id, services } => {
            let services: Vec<String> = services.into_iter().map(|s| s.to_short_string()).collect();
            debug!("ServicesAdvertisement: {:?}, {:?}", id, services);
        }
        _ => {}
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let mut stream_map: StreamMap<StreamKey, EventStreams> = StreamMap::new();

    // let service_uuids: Vec<Uuid> = vec![
    //     // "69400001-B5A3-F393-E0A9-E50E24DCCA99".parse().unwrap(),
    //     // "7F510004-B5A3-F393-E0A9-E50E24DCCA9E".parse().unwrap(),
    //     // "69400002-B5A3-F393-E0A9-E50E24DCCA99".parse().unwrap(),
    //     // "69400003-B5A3-F393-E0A9-E50E24DCCA99".parse().unwrap(),

    //     // "0000fe9f-0000-1000-8000-00805f9b34fb".parse().unwrap(),
    //     // uuid_from_u16(0x1800),
    //     // uuid_from_u16(0x1801)
    //     // uuid_from_u16(0x180A)
    // ];

    let manager = Manager::new().await.unwrap();

    // get the first bluetooth adapter
    let central = manager
        .adapters()
        .await
        .expect("Unable to fetch adapter list.")
        .into_iter()
        .nth(0)
        .expect("Unable to find adapters.");

    // Each adapter has an event stream, we fetch via events(),
    // simplifying the type, this will return what is essentially a
    // Future<Result<Stream<Item=CentralEvent>>>.

    let btle_events = central.events().await?;
    stream_map.insert(StreamKey::BtleEvents, EventStreams::BtleEvents(btle_events));

    debug!("Discovering...");

    // start scanning for devices
    central.start_scan(ScanFilter::default()).await?;
    // central
    //     .start_scan(ScanFilter {
    //         services: service_uuids,
    //     })
    //     .await?;

    // instead of waiting, you can use central.events() to get a stream which will
    // notify you of new devices, for an example of that see examples/event_driven_discovery.rs
    // time::sleep(Duration::from_secs(2)).await;

    // Print based on whatever the event receiver outputs. Note that the event
    // receiver blocks, so in a real program, this should be run in its own
    // thread (not task, as this library does not yet use async channels).

    loop {
        let event = stream_map.next().await;
        match event {
            Some((StreamKey::BtleEvents, EventVariants::Event(event))) => {
                handle_btle_event(
                    &event,
                    &central,
                    // &mut peripherals,
                    // &mut disconnected,
                    &mut stream_map,
                    // tx.clone(),
                )
                .await?;
            }
            Some((StreamKey::BtleNotifications(id), EventVariants::Notification(value))) => {
                handle_btle_notification(&central, &id, &value).await?;
            }
            Some((_, _)) => unreachable!(),
            None => todo!(),
        }
    }

    // Ok(())
}
