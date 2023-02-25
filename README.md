# Unofficial bindings for the Proton REST API in Rust

This project aims to implement the Proton REST API in Rust. It is all based on the information available
from the [go-proton-api](https://github.com/ProtonMail/go-proton-api) and the
[Proton Bridge](https://github.com/ProtonMail/proton-bridge) repositories.

## Disclaimer

This is an **UNOFFICIAL** project and a **work in progress**.  Use the code in this repository at your own risk. The
author of this project will not be held liable if data loss occurs or your account gets blocked.

## Build Requirements 

* Rust 
* Go >= 1.19

This library currently uses one go library to handle the SRP part of the authentication. While there are srp crates 
available for rust, to avoid issues with the proton servers, we currently use the library that's used internally by
[go-proton-api](https://github.com/ProtonMail/go-proton-api).

## Safety

This project currently needs unsafe to interact with the go bindings for srp