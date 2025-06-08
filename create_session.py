from telethon import TelegramClient
from dotenv import load_dotenv
import os
import sys

# Load environment variables
load_dotenv()

API_ID = int(os.environ["TG_API_ID"])
API_HASH = os.environ["TG_API_HASH"]
PHONE = os.environ["TG_PHONE"]

async def main():
    print("Creating Telegram session...")
    print(f"Using phone: {PHONE}")
    
    client = TelegramClient('test_session', API_ID, API_HASH)
    
    try:
        await client.connect()
        
        if not await client.is_user_authorized():
            print("Sending code request...")
            await client.send_code_request(PHONE)
            code = input('Enter the code you received: ')
            await client.sign_in(PHONE, code)
        
        print("Session file created successfully!")
        print("You can now run the tests.")
        
    except Exception as e:
        print(f"Error: {str(e)}", file=sys.stderr)
        sys.exit(1)
    finally:
        await client.disconnect()

if __name__ == '__main__':
    import asyncio
    asyncio.run(main()) 