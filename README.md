## Overview
This repo is made for learning purposes. Through doing this project I want to learn about the following topics:
* Intro to Bevy
* Multithreading in Bevy (and sharing of states between a Bevy client and an external process)
* Basic use of serde in conjunction with UDP
* Tokio and async (this is in the udp server)

There are two parts to this repo, udp_server and game:
* udp_server is the server that handles the game states and the communication with one or more clients.
* game is the client that displays what is happening. In this case it's just position of sprites. It also allows the player to control the position of their own sprite. 

## game
A Bevy client with the following responsibilities:
* Connects to a server
* Sends a message to the server
* Receives a message from the server
* Allows position manipulation of its own sprites
* Takes message sent from the server and displays it on the same screen

### Quirks and Learnings
I did not want to hard couple the sending of information to the rendering of each frame. I want the sending and receiving of information to be decoupled from the rendering. 
Therefore there are two loops that runs _outside_ of the game loop that exclusively handles the communication:
* Internal facing loop
  * This loop listens for channel messages sent from within the client  
  * Upon receiving a message here it will either update the server via sending a udp packet or it updates an internal state to inform the game loop that a position of an asset that is not governed by this instance of the client that it has changed
* External facing loop
  * This loop listens for incoming udp packets sent from the udp_server
  * These are either position change message (messages that tell the client that position of a sprite has changed), or they are player insertion message (message telling the client to start rendering and keep track of a sprite that is previously not known to the client)
