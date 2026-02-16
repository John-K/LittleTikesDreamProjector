# Known Cartridge Hashes

Measured using `shasum <file.bin>`

| Cartridge | SHA-1 Hash |
| --------- | ---------- |
| Apple Picking Day | `e037b548b761535a01224b9f22d12cf2827cacfd` |
| Beach Day | `f6921b4e022a5269b9da7e017967d8a554d1424a` |
| Bernstein Bears - Catch The Bus | `7b943d0ec3392097620960ca61f5a889a92fe1a0` |
| Bernstein Bears - Big Bear, Small Bear | `6a19a116dfab5638ef629baf97c846b38f8b4e7e` |
| Fraggle Rock - Fraggle Countdown | `d397b68c8685d662817b6435aeed43e08c0ae051` |
| Fraggle Rock - Wembley Fraggle's Big, Bigger, Biggest | `8891735769613439a542daea60fec16efd844f8d` |
| Fraggle Rock - What's A Fraggle | `a102e7a716ed809395a320f54f182fca3accb22d` |
| Grandparent's Day | `3af15e3026e3debd994d4abe0444c33dc29aaa18` |
| Little Golden - Tawny Scrawny Lion | `b200bd8ebf67ff4f335ebc8ede4fa85e5c9eb779` |
| Little Golden - The Poky Little Puppy | `5b04b587d1393a3b0c71d5404296db6b8ae3139d` |
| Little Golden - The Saggy Baggy Elephant | `8d831becff97cdc4afa51054f994b775bf7855cd` |
| Paw Patrol - All-Star Pups | `cf0b144c22194c3e06a7bb50b351d16fa36a29ff` |
| Paw_Patrol - Puppy Dance Party | `91524ef808539e778c194408b888f426603d602e` |
| Paw_Patrol - Save The_School Bus | `691f541bf0dc60fb32a0c448ce6e72b26231dd8b` |
| Pokey Little Puppy | `5b04b587d1393a3b0c71d5404296db6b8ae3139d` |
| Pokey Little Puppy (Non-empty unused flash) | `61d76e198b523601cc44aa84ebc1b7033405bffc` |

> [!NOTE]
> The "Non-empty unused flash" variant of Pokey Little Puppy contains the exact same data as the normal cart, but the empty space is not filled with `FF` bytes as is the case with other carts.
>
> The empty space is mostly filled with `80` with some sort of data at the beginning of the empty space and a different "ID".
>
> The 16-byte "ID" near the end of the flash:
> * Regular:
>   * `000fff80: 0203 0003 0003 0001 0203 0101 0201 0006`
>   * `000fff90: FFFF FFFF FFFF FFFF FFFF FFFF FFFF FFFF`
> * Variant: 
>   * `000fff80: 5600 0100 0300 0302 034c 0005 0009 0102`
>   * `000fff90: 0203 ff80 8080 8080 8080 8080 8080 8080`
>
> The back of the cart has the markings `W33K1 31` behind the edge connector
