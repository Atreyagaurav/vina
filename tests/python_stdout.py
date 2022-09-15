# import asyncio
import time

# async def main():
#     for i in range(101):
#         if i == 50:
#             await asyncio.sleep(1)
#         with open("./.log", "a") as w:
#             w.write(f"Testing: {i}")
#         print(f"Testing: {i}", flush=True)

#     await asyncio.sleep(3)


def main():
    for i in range(110):
        if i == 50:
            time.sleep(1)
        print(f"Nice: {i}", flush=True)
        print(f"What: {i//2}", flush=True)
    time.sleep(3)


if __name__ == '__main__':
    # asyncio.run(main())
    main()
