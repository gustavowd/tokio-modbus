// SPDX-FileCopyrightText: Copyright (c) 2017-2023 slowtec GmbH <post@slowtec.de>
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    fmt,
    io::{Error, ErrorKind},
};

//use gpio::GpioOut;
use rppal::gpio::{Gpio, OutputPin};

use futures_util::{sink::SinkExt as _, stream::StreamExt as _};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

use crate::{
    codec,
    frame::{rtu::*, *},
    slave::*,
};

/// Modbus RTU client
#[derive(Debug)]
pub(crate) struct Client<T> {
    framed: Framed<T, codec::rtu::ClientCodec>,
    slave_id: SlaveId,
}

impl<T> Client<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub(crate) fn new(transport: T, slave: Slave) -> Self {
        let framed = Framed::new(transport, codec::rtu::ClientCodec::default());
        let slave_id = slave.into();
        Self { framed, slave_id }
    }

    fn next_request_adu<R>(&self, req: R, disconnect: bool) -> RequestAdu
    where
        R: Into<RequestPdu>,
    {
        let slave_id = self.slave_id;
        let hdr = Header { slave_id };
        let pdu = req.into();
        RequestAdu {
            hdr,
            pdu,
            disconnect,
        }
    }

    async fn call(&mut self, req: Request) -> Result<Response, Error> {
        let disconnect = req == Request::Disconnect;
        let req_adu = self.next_request_adu(req, disconnect);
        let req_hdr = req_adu.hdr;

        // get gpio pin to control modbus bus
        let mut de_pin: Option<OutputPin> = None;
        let mut re_pin: Option<OutputPin> = None;
        //let mut de_pin = gpio::sysfs::SysFsGpioOutput::open(23);
        //let mut re_pin = gpio::sysfs::SysFsGpioOutput::open(24);
        let de_port = Gpio::new();
        let re_port = Gpio::new();//.unwrap().get(24).unwrap().into_output();
        match de_port {
            Ok (de) => {
                match de.get(23) {
                    Ok (de) => {
                        de_pin = Some(de.into_output());
                    },
                    Err(_) => {}
                }

            },
            Err(_) => {}
        }

        match re_port {
            Ok (re) => {
                match re.get(23) {
                    Ok (re) => {
                        re_pin = Some(re.into_output());
                    },
                    Err(_) => {}
                }

            },
            Err(_) => {}
        }

        match de_pin.as_mut() {
            Some(de) => de.set_high(),
            None => {},
        }
        match re_pin.as_mut() {
            Some(re) => re.set_high(),
            None => {},
        }
        /* 
        match de_pin.as_mut() {
            Ok(gpio) => gpio.set_high().expect("could not set gpio 23"),
            Err(_) => {},
        }
        match re_pin.as_mut() {
            Ok(gpio) => gpio.set_high().expect("could not set gpio 24"),
            Err(_) => {},
        }
        */

        self.framed.send(req_adu).await?;

        match de_pin.as_mut() {
            Some(de) => de.set_low(),
            None => {},
        }
        match re_pin.as_mut() {
            Some(re) => re.set_low(),
            None => {},
        }
        /* 
        match de_pin.as_mut() {
            Ok(gpio) => gpio.set_low().expect("could not set gpio 23"),
            Err(_) => {},
        }
        match re_pin.as_mut() {
            Ok(gpio) => gpio.set_low().expect("could not set gpio 24"),
            Err(_) => {},
        }
        */

        let res_adu = self
            .framed
            .next()
            .await
            .unwrap_or_else(|| {
                /*
                match de_pin.as_mut() {
                    Ok(gpio) => gpio.set_high().expect("could not set gpio 23"),
                    Err(_) => {},
                }
                match re_pin.as_mut() {
                    Ok(gpio) => gpio.set_high().expect("could not set gpio 24"),
                    Err(_) => {},
                }
                */
                match de_pin.as_mut() {
                    Some(de) => de.set_high(),
                    None => {},
                }
                match re_pin.as_mut() {
                    Some(re) => re.set_high(),
                    None => {},
                }
                Err(Error::from(ErrorKind::BrokenPipe))
            }
            )?;

        /*
        match de_pin.as_mut() {
            Ok(gpio) => gpio.set_high().expect("could not set gpio 23"),
            Err(_) => {},
        }
        match re_pin.as_mut() {
            Ok(gpio) => gpio.set_high().expect("could not set gpio 24"),
            Err(_) => {},
        }
        */
        match de_pin.as_mut() {
            Some(de) => de.set_high(),
            None => {},
        }
        match re_pin.as_mut() {
            Some(re) => re.set_high(),
            None => {},
        }

        drop(de_pin);
        drop(re_pin);

        match res_adu.pdu {
            ResponsePdu(Ok(res)) => verify_response_header(req_hdr, res_adu.hdr).and(Ok(res)),
            ResponsePdu(Err(err)) => Err(Error::new(ErrorKind::Other, err)),
        }
    }
}

fn verify_response_header(req_hdr: Header, rsp_hdr: Header) -> Result<(), Error> {
    if req_hdr != rsp_hdr {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "Invalid response header: expected/request = {req_hdr:?}, actual/response = {rsp_hdr:?}"
            ),
        ));
    }
    Ok(())
}

impl<T> SlaveContext for Client<T> {
    fn set_slave(&mut self, slave: Slave) {
        self.slave_id = slave.into();
    }
}

#[async_trait::async_trait]
impl<T> crate::client::Client for Client<T>
where
    T: fmt::Debug + AsyncRead + AsyncWrite + Send + Unpin,
{
    async fn call(&mut self, req: Request) -> Result<Response, Error> {
        self.call(req).await
    }
}

#[cfg(test)]
mod tests {

    use core::{
        pin::Pin,
        task::{Context, Poll},
    };
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, Result};

    #[derive(Debug)]
    struct MockTransport;

    impl Unpin for MockTransport {}

    impl AsyncRead for MockTransport {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut ReadBuf<'_>,
        ) -> Poll<Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for MockTransport {
        fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, _: &[u8]) -> Poll<Result<usize>> {
            Poll::Ready(Ok(2))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn handle_broken_pipe() {
        let transport = MockTransport;
        let mut client =
            crate::service::rtu::Client::new(transport, crate::service::rtu::Slave::broadcast());
        let res = client
            .call(crate::service::rtu::Request::ReadCoils(0x00, 5))
            .await;
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
    }
}
