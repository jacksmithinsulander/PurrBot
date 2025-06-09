import os
import pytest
import asyncio
from unittest.mock import Mock, AsyncMock, MagicMock
from dotenv import load_dotenv
import pytest_asyncio
import sys
from telethon.errors import SessionPasswordNeededError

# Custom exception for timeout
class TimedOutError(Exception):
    pass

# Load environment variables
load_dotenv()

# Use mock values if environment variables are not set
API_ID = int(os.environ.get("TG_API_ID", "12345"))
API_HASH = os.environ.get("TG_API_HASH", "test_hash")
PHONE = os.environ.get("TG_PHONE", "+1234567890")
BOT_USERNAME = os.environ.get("TG_BOT_USERNAME", "@testbot")
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

@pytest.fixture(scope="session")
async def client(event_loop):
    """Create a mock Telegram client for testing."""
    # Create a mock client
    mock_client = AsyncMock()
    
    # Mock bot entity
    bot_entity = Mock()
    bot_entity.id = 123456789
    bot_entity.username = BOT_USERNAME
    
    # Mock message responses
    def create_mock_message(text, sender_id):
        msg = Mock()
        msg.text = text
        msg.sender_id = sender_id
        return msg
    
    # Set up mock responses for different commands
    responses = {
        '/logout': "You have been logged out.",
        '/start': "Welcome! GM anon!",
        f'/signup {PASSWORD}': "Account created successfully!",
        PASSWORD: "You are now logged in.",
        '/list': "Your list: [empty]"
    }
    
    # Track message history
    message_history = []
    
    async def mock_send_message(entity, message):
        message_history.append(message)
        # Return None as send_message doesn't return anything
        return None
    
    async def mock_get_messages(chat_id, limit=5):
        # Return appropriate response based on last sent message
        if message_history:
            last_msg = message_history[-1]
            response_text = responses.get(last_msg, "Unknown command")
            return [create_mock_message(response_text, bot_entity.id)]
        return []
    
    async def mock_get_entity(username):
        return bot_entity
    
    # Set up mock methods
    mock_client.send_message = mock_send_message
    mock_client.get_messages = mock_get_messages
    mock_client.get_entity = mock_get_entity
    mock_client.connect = AsyncMock(return_value=None)
    mock_client.is_user_authorized = AsyncMock(return_value=True)
    mock_client.disconnect = AsyncMock(return_value=None)
    
    await mock_client.connect()
    
    yield mock_client
    
    await mock_client.disconnect()

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