use async_trait::async_trait;
use pnet_base::MacAddr;
use pnet_packet::Packet;
use std::net::Ipv4Addr;
use tokio::sync::broadcast;
use tokio_stream::Stream;

#[tokio::main]
async fn main() {
    let (tx, rx1) = broadcast::channel(16);

    let ether_1 = Ether {
        sender: tx.clone(),
        receiver: rx1,
    };

    let ip_1 = IP {
        inner_protocol: ether_1,
        address_translator: |ipv4_address: Ipv4Addr| match ipv4_address.octets() {
            [192, 16, 16, 16] => MacAddr::new(10, 12, 14, 16, 18, 20),
            [190, 16, 16, 16] => MacAddr::new(12, 12, 14, 16, 18, 20),
            _ => panic!("Unknown Address!!!!!!!!!!!!!!!!!"),
        },
    };

    let udp_1 = UDP {
        inner_protocol: ip_1,
    };

    let ether_2 = Ether {
        receiver: tx.subscribe(),
        sender: tx,
    };

    let ip_2 = IP {
        inner_protocol: ether_2,
        address_translator: |ipv4_address: Ipv4Addr| match ipv4_address.octets() {
            [192, 16, 16, 16] => MacAddr::new(10, 12, 14, 16, 18, 20),
            [190, 16, 16, 16] => MacAddr::new(12, 12, 14, 16, 18, 20),
            _ => panic!("Unknown Address!!!!!!!!!!!!!!!!!"),
        },
    };

    let udp_2 = UDP {
        inner_protocol: ip_2,
    };

    let connection_1 = udp_1.connect(
        (Ipv4Addr::new(192, 16, 16, 16), 80),
        (Ipv4Addr::new(190, 16, 16, 16), 80),
        (),
    );
    let data = vec![1, 2, 4];

    let mut connection_2 = udp_2.connect(
        (Ipv4Addr::new(190, 16, 16, 16), 80),
        (Ipv4Addr::new(192, 16, 16, 16), 80),
        (),
    );
    UDP::send(&connection_1, &data).await;
    println!("Hello");
    let mut buffer = Vec::new();
    UDP::receive(&mut connection_2, &mut buffer).await;
    println!("{:?}", buffer);
}
// Use of this trait allows the functions to be async. The reason why the functions should be
// async is so that I do not have to deal with multithreading.
#[async_trait]
pub trait Protocol {
    type Address: Copy + Send + Sync;
    type Connection: Send + Sync;
    type ConnectionConfig;

    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection;
    async fn receive(connection: &mut Self::Connection, buffer: &mut Vec<u8>);
    async fn send(connection: &Self::Connection, data: &[u8]);
}
#[async_trait]
pub trait Bind: Protocol {
    type ServerConnection: Send + Sync;
    fn bind(address: Self::Address) -> Self::ServerConnection;
    async fn listen(connection: &mut Self::ServerConnection) -> Self::Connection;
}

// Currently failure is not built into the system, so like what happens if it fails to send etc
// So, the thing with UDP is that ut us really meant to send individual packets, and not streams
// of data, so we only need to worry about a packet at a time.
#[async_trait]
impl<P: Protocol + SupportedConfiguration<Self>> Protocol for UDP<P>
where
    Self: UDPChecksum<P>,
{
    type Address = (P::Address, u16);
    type Connection = UDPConnection<P>;
    type ConnectionConfig = ();

    fn connect(
        &self,
        (source_address, source_port): Self::Address,
        (destination_address, destination_port): Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection {
        UDPConnection {
            connection: self.inner_protocol.connect(
                source_address,
                destination_address,
                P::get_config(),
            ),
            source_port,
            destination_port,
            destination_address,
            source_address,
        }
    }

    async fn receive(connection: &mut Self::Connection, buffer: &mut Vec<u8>) {
        loop {
            // Here i should maybe just use a new vector, because I want a mut buffer, and I
            // can't keep on handing stuff out... but lets try it first.
            // So after recieve I have the buffer, and now I can maybe use it to make the new packet
            let mut underlying_buffer = Vec::new();
            P::receive(&mut connection.connection, &mut underlying_buffer).await;
            let packet = pnet_packet::udp::UdpPacket::new(&underlying_buffer).unwrap(); // <---
                                                                                        // seems to be ok for now, but might have issues?
            println!("{:?}", packet);
            if packet.get_source() == connection.destination_port
                && packet.get_destination() == connection.source_port
            {
                return buffer.append(&mut packet.payload().to_vec());
            }
        }
    }

    async fn send(connection: &Self::Connection, data: &[u8]) {
        let packet_buffer = vec![0; 8 + data.len()];
        let packet_total_len = packet_buffer.len();
        let mut packet = pnet_packet::udp::MutableUdpPacket::owned(packet_buffer).unwrap();
        packet.set_source(connection.source_port);
        packet.set_destination(connection.destination_port);
        packet.set_length(packet_total_len as u16);
        packet.set_payload(data);
        let checksum = Self::calculate_checksum(
            packet.to_immutable(),
            connection.source_address,
            connection.destination_address,
        );
        packet.set_checksum(checksum);
        let packet_vec = packet.packet().to_vec();
        P::send(&connection.connection, &packet_vec).await;
    }
}

pub trait UDPChecksum<P: Protocol> {
    fn calculate_checksum(
        packet: pnet_packet::udp::UdpPacket,
        source_address: P::Address,
        destination_address: P::Address,
    ) -> u16;
}

impl<P: Protocol + SupportedConfiguration<IP<P, F>>, F: Fn(Ipv4Addr) -> P::Address, T>
    UDPChecksum<IP<P, F>> for UDP<T>
{
    fn calculate_checksum(
        packet: pnet_packet::udp::UdpPacket,
        source_address: <IP<P, F> as Protocol>::Address,
        destination_address: <IP<P, F> as Protocol>::Address,
    ) -> u16 {
        pnet_packet::udp::ipv4_checksum(&packet, &source_address, &destination_address)
    }
}

pub struct UDPConnection<P: Protocol> {
    connection: P::Connection,
    source_port: u16,
    destination_port: u16,
    destination_address: P::Address,
    source_address: P::Address,
}

pub struct EtherConnectionConfig {
    ether_type: pnet_packet::ethernet::EtherType,
}

pub struct IPConnectionConfig {
    protocol: pnet_packet::ip::IpNextHeaderProtocol,
}

pub trait SupportedConfiguration<P>: Protocol {
    fn get_config() -> Self::ConnectionConfig;
}

impl<P, F> SupportedConfiguration<IP<P, F>> for Ether {
    fn get_config() -> Self::ConnectionConfig {
        EtherConnectionConfig {
            ether_type: pnet_packet::ethernet::EtherTypes::Ipv4,
        }
    }
}

#[async_trait]
impl Protocol for Ether {
    type Address = MacAddr;
    type Connection = EtherConnection;
    type ConnectionConfig = EtherConnectionConfig;

    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection {
        EtherConnection {
            source_mac: source_address,
            destination_mac: destination_address,
            config: connection_config,
            sender: self.sender.clone(),
            receiver: self.sender.subscribe(),
        }
    }

    async fn receive(connection: &mut Self::Connection, buffer: &mut Vec<u8>) {
        loop {
            let data = connection.receiver.recv().await.unwrap();
            let packet = pnet_packet::ethernet::EthernetPacket::new(&data).unwrap();
            let packet_source = packet.get_source();
            let packet_destination = packet.get_destination();
            if packet_source == connection.destination_mac
                && packet_destination == connection.source_mac
                && packet.get_ethertype() == connection.config.ether_type
            {
                buffer.append(&mut packet.payload().to_vec());
                return;
            }
            println!("Ether");
        }
    }

    async fn send(connection: &Self::Connection, data: &[u8]) {
        let packet_buffer = vec![0; 22 + data.len()];
        let mut packet =
            pnet_packet::ethernet::MutableEthernetPacket::owned(packet_buffer).unwrap();
        packet.set_source(connection.source_mac);
        packet.set_destination(connection.destination_mac);
        packet.set_ethertype(connection.config.ether_type);
        packet.set_payload(data);
        connection.sender.send(packet.packet().to_vec()).unwrap();
    }
}

impl<P: Protocol + SupportedConfiguration<Self>, F: Fn(Ipv4Addr) -> P::Address, T>
    SupportedConfiguration<UDP<T>> for IP<P, F>
{
    fn get_config() -> Self::ConnectionConfig {
        IPConnectionConfig {
            protocol: pnet_packet::ip::IpNextHeaderProtocols::Udp,
        }
    }
}

#[async_trait]
impl<P: Protocol + SupportedConfiguration<Self>, F: Fn(Ipv4Addr) -> P::Address> Protocol
    for IP<P, F>
{
    type Address = Ipv4Addr;
    type Connection = IPConnection<P>;
    type ConnectionConfig = IPConnectionConfig;

    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection {
        IPConnection {
            connection: self.inner_protocol.connect(
                (self.address_translator)(source_address),
                (self.address_translator)(destination_address),
                P::get_config(),
            ),
            source_ip: source_address,
            destination_ip: destination_address,
            config: connection_config,
        }
    }

    async fn receive(connection: &mut Self::Connection, buffer: &mut Vec<u8>) {
        loop {
            let mut underlying_buffer = Vec::new();
            P::receive(&mut connection.connection, &mut underlying_buffer).await;
            println!("Post Ether");
            let packet = pnet_packet::ipv4::Ipv4Packet::new(&underlying_buffer).unwrap();
            println!("{:?}", packet);
            if packet.get_source() == connection.destination_ip
                && packet.get_destination() == connection.source_ip
                && connection.config.protocol == packet.get_next_level_protocol()
            {
                return buffer.append(&mut packet.payload().to_vec());
            }
        }
    }

    async fn send(connection: &Self::Connection, data: &[u8]) {
        let packet_buffer = vec![0; 20 + data.len()];
        let packet_total_len = packet_buffer.len();
        let mut packet = pnet_packet::ipv4::MutableIpv4Packet::owned(packet_buffer).unwrap();
        packet.set_version(4);
        packet.set_header_length(5);
        packet.set_total_length(packet_total_len as u16);
        packet.set_next_level_protocol(connection.config.protocol);
        packet.set_source(connection.source_ip);
        packet.set_destination(connection.destination_ip);
        packet.set_payload(data);
        let checksum = pnet_packet::ipv4::checksum(&packet.to_immutable());
        packet.set_checksum(checksum);
        let packet_vec = packet.packet().to_vec();
        P::send(&connection.connection, &packet_vec).await;
    }
}

pub struct IPConnection<P: Protocol> {
    connection: P::Connection,
    source_ip: Ipv4Addr,
    destination_ip: Ipv4Addr,
    config: IPConnectionConfig,
}

pub struct EtherConnection {
    source_mac: MacAddr,
    destination_mac: MacAddr,
    config: EtherConnectionConfig,
    sender: tokio::sync::broadcast::Sender<Vec<u8>>,
    receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
}

pub struct Ether {
    sender: tokio::sync::broadcast::Sender<Vec<u8>>,
    receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
}

pub struct UDP<P> {
    inner_protocol: P,
}

pub struct IP<P, F> {
    inner_protocol: P,
    address_translator: F,
}
