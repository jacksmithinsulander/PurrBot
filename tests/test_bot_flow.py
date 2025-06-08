import os
import pytest
import asyncio
from telethon import TelegramClient
from dotenv import load_dotenv
import pytest_asyncio
import sys
import time

# Load environment variables
load_dotenv()

API_ID = int(os.environ["TG_API_ID"])
API_HASH = os.environ["TG_API_HASH"]
PHONE = os.environ["TG_PHONE"]
BOT_USERNAME = os.environ["TG_BOT_USERNAME"]
PASSWORD = os.environ.get("TG_TEST_PASSWORD", "testpassword")

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

@pytest.mark.asyncio
async def test_bot_signup_flow(client):
    """Test the complete signup flow with the bot."""
    try:
        # Wait for bot to be ready
        print("Waiting for bot to be ready...")
        time.sleep(5)  # Give the bot more time to start up
        
        async with client.conversation(BOT_USERNAME, timeout=15) as conv:
            # Test start command
            await conv.send_message('/start')
            resp = await conv.get_response()
            assert "welcome" in resp.text.lower(), "Bot should welcome the user"

            # Test signup command with password
            await conv.send_message(f'/signup {PASSWORD}')
            resp = await conv.get_response()
            assert "success" in resp.text.lower(), "Signup should be successful"

            # Test list command
            await conv.send_message('/list')
            resp = await conv.get_response()
            assert "list" in resp.text.lower(), "Bot should show list"

            # Test logout
            await conv.send_message('/logout')
            resp = await conv.get_response()
            assert "logged out" in resp.text.lower(), "Logout should be successful"
    except Exception as e:
        pytest.fail(f"Test failed with error: {str(e)}")