import init, { make_reservation, Cli } from "./reserve.js"

init().then(async () => {
  window.cli = (name, guests, email, phone, day, time, instructions = null) =>
    new Cli(name, guests, email, phone, day, time, instructions)
  window.make_reservation = make_reservation
  console.log("WebAssembly module initialized.")
})
