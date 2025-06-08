import os
import pytest
import asyncio
from telethon import TelegramClient
from dotenv import load_dotenv
import pytest_asyncio
import sys
from telethon.errors import SessionPasswordNeededError

# Custom exception for timeout
class TimedOutError(Exception):
    pass

# Load environment variables
load_dotenv()

API_ID = int(os.environ["TG_API_ID"])
API_HASH = os.environ["TG_API_HASH"]
PHONE = os.environ["TG_PHONE"]
BOT_USERNAME = os.environ["TG_BOT_USERNAME"]
PASSWORD = os.environ.get("TG_TEST_PASSWORD", "testpassword")

async def wait_for_response(client, chat_id, bot_id, timeout=30):
    """Wait for a response from the bot with a timeout."""
    start_time = asyncio.get_event_loop().time()
    while True:
        try:
            # Get the last message from the chat
            messages = await client.get_messages(chat_id, limit=5)
            for msg in messages:
                if msg.sender_id == bot_id:
                    return msg
        except Exception as e:
            print(f"Error while waiting for response: {e}")
        # Check if we've exceeded the timeout
        if asyncio.get_event_loop().time() - start_time > timeout:
            raise TimedOutError("Timed out waiting for bot response")
        # Wait a bit before trying again
        await asyncio.sleep(0.5)

@pytest.fixture(scope="session")
def event_loop():
    """Create an instance of the default event loop for the test session."""
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    yield loop
    loop.close()

@pytest_asyncio.fixture(scope="session")
async def client(event_loop):
    """Create a Telegram client for testing."""
    session_name = 'test_session'
    client = TelegramClient(session_name, API_ID, API_HASH)
    
    try:
        await client.connect()
        if not await client.is_user_authorized():
            print("\nError: No valid session found!", file=sys.stderr)
            print("Please run this command first:", file=sys.stderr)
            print("docker compose run test python tests/create_session.py", file=sys.stderr)
            sys.exit(1)
            
        yield client
    finally:
        await client.disconnect()

@pytest.mark.asyncio(scope="session")
async def test_bot_signup_flow(client):
    """Test the complete signup flow with the bot."""
    try:
        # Wait for bot to be ready
        print("Waiting for bot to be ready...")
        await asyncio.sleep(5)  # Give the bot more time to start up
        
        # Get the bot's chat ID
        bot_entity = await client.get_entity(BOT_USERNAME)
        chat_id = bot_entity.id
        bot_id = bot_entity.id
        
        # Ensure clean state by logging out first
        print("Sending /logout command...")
        await client.send_message(bot_entity, '/logout')
        print("Waiting for /logout response...")
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Received response to /logout: {resp.text}")
        assert "logged out" in resp.text.lower(), "Logout should be successful"
        await asyncio.sleep(2)  # Add longer delay after logout

        # Test start command
        print("Sending /start command...")
        await client.send_message(bot_entity, '/start')
        print("Waiting for /start response...")
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Received response to /start: {resp.text}")
        assert "welcome" in resp.text.lower() or "gm anon" in resp.text.lower(), "Bot should welcome the user"
        await asyncio.sleep(1)  # Add delay between commands

        # Test signup command with password
        print("Sending /signup command...")
        await client.send_message(bot_entity, f'/signup {PASSWORD}')
        print("Waiting for /signup response...")
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Received response to /signup: {resp.text}")
        assert "account created" in resp.text.lower(), "Signup should be successful and ask for password again"
        await asyncio.sleep(1)  # Add delay between commands

        # Send password again to log in
        print("Sending password for login...")
        await client.send_message(bot_entity, PASSWORD)
        print("Waiting for login response...")
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Received response to login: {resp.text}")
        assert "logged in" in resp.text.lower(), "Login after signup should be successful"
        await asyncio.sleep(1)  # Add delay between commands

        # Test list command
        print("Sending /list command...")
        await client.send_message(bot_entity, '/list')
        print("Waiting for /list response...")
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Received response to /list: {resp.text}")
        assert "list" in resp.text.lower(), "Bot should show list"
        await asyncio.sleep(1)  # Add delay between commands

        # Test logout
        print("Sending /logout command...")
        await client.send_message(bot_entity, '/logout')
        print("Waiting for /logout response...")
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Received response to /logout: {resp.text}")
        assert "logged out" in resp.text.lower(), "Logout should be successful"
        await asyncio.sleep(2)  # Add longer delay after logout

        # Test start command after logout
        print("Sending /start command after logout...")
        await client.send_message(bot_entity, '/start')
        print("Waiting for /start response after logout...")
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Received response to /start after logout: {resp.text}")
        assert "welcome" in resp.text.lower() or "gm anon" in resp.text.lower(), "Bot should welcome the user after logout"
    except Exception as e:
        print(f"Test failed with error: {str(e)}")
        pytest.fail(f"Test failed with error: {str(e)}")