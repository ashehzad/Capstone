use std::marker::PhantomData;
use std::ops::{Add, AddAssign};

fn main() {}

pub trait Protocol<Data> {
    type Address;
    type Connection: Sender<Data>;
    fn connect(
        address: Self::Address,
        connection_handler: impl Fn(Self::Connection),
        data_handler: impl Fn(Self::Connection, Data),
    );
}

pub trait Sender<Data> {
    fn send(data: Data);
}

pub trait Transport<Data>: Protocol<Data> {
    type Port;
    type Checksum;
}

pub struct UDP<Address> {
    _address: PhantomData<Address>,
}

pub struct UDPHeader {
    source_port: u16,
    destination_port: u16,
    length: u16,
    checksum: u16,
}

impl<Data, Address> Transport<Data> for UDP<Address> {
    type Port = u16;
    type Checksum = u16;
}

impl<Data, Address> Protocol<Data> for UDP<Address> {
    type Address = (Address, <Self as Transport<Data>>::Port);
    type Connection = ();

    fn connect(
        address: Self::Address,
        connection_handler: impl Fn(Self::Connection),
        data_handler: impl Fn(Self::Connection, Data),
    ) {
        todo!()
    }
}
