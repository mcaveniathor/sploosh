# Sploosh
## A needlessly-performant timing server for activating GPIO outputs

At the core this program is nothing but a timer that periodically turns on a switch for a given duration at a given time of day; in my case, I'm using it to power a relay, and in turn a solenoid valve, as an irrigation controller for my backyard garden running on an Odroid C4 SBC. 
Included is both the multithreaded and asynchronous timer functionality alongside a basic but functional HTTP interface to add additional timers and an embedded database in which to store timer configurations. 
