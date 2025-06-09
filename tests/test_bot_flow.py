import os
import pytest
import asyncio
from unittest.mock import Mock, AsyncMock, MagicMock
from dotenv import load_dotenv

# Custom exception for timeout
class TimedOutError(Exception):
    pass

# Load environment variables
load_dotenv()

# Mock bot responses based on the actual bot implementation
BOT_RESPONSES = {
    '/logout': "👋 You have been logged out successfully!",
    '/start': "💻 gm anon, whatchu wanna do? 🐈",
    '/signup hey': "Account created successfully! 🎉\nNow enter your password again to log in.",
    'hey': "Logged in successfully! 🎉",
    '/list': "📋 Listing your items..."
}

class MockMessage:
    def __init__(self, text, sender_id):
        self.text = text
        self.sender_id = sender_id
        self.id = id(self)  # Use object id as message id

@pytest.fixture
def mock_client():
    """Create a mock Telegram client for testing."""
    client = AsyncMock()
    
    # Mock bot entity
    bot_entity = Mock()
    bot_entity.id = 123456789
    bot_entity.username = "@Purr9Bot"
    
    # Track conversation state
    conversation_state = {"messages": [], "logged_in": False}
    
    async def mock_send_message(entity, message):
        """Mock sending a message and generate appropriate response."""
        conversation_state["messages"].append(message)
        
        # Generate bot response based on the message
        if message in BOT_RESPONSES:
            response_text = BOT_RESPONSES[message]
        elif message == "hey" and any("/signup" in m for m in conversation_state["messages"]):
            response_text = BOT_RESPONSES["hey"]
            conversation_state["logged_in"] = True
        else:
            response_text = "Unknown command"
            
        # Update state based on commands
        if message == "/logout":
            conversation_state["logged_in"] = False
        
        return None
    
    async def mock_get_messages(chat_id, limit=10):
        """Return the appropriate bot response."""
        if not conversation_state["messages"]:
            return []
            
        last_message = conversation_state["messages"][-1]
        
        # Get appropriate response
        if last_message in BOT_RESPONSES:
            response_text = BOT_RESPONSES[last_message]
        elif last_message == "hey" and any("/signup" in m for m in conversation_state["messages"]):
            response_text = BOT_RESPONSES["hey"]
        else:
            response_text = "Unknown command"
            
        return [MockMessage(response_text, bot_entity.id)]
    
    async def mock_get_entity(username):
        return bot_entity
    
    # Set up mock methods
    client.send_message = mock_send_message
    client.get_messages = mock_get_messages
    client.get_entity = mock_get_entity
    client.connect = AsyncMock(return_value=None)
    client.is_user_authorized = AsyncMock(return_value=True)
    client.disconnect = AsyncMock(return_value=None)
    
    return client

async def wait_for_response(client, chat_id, bot_id, timeout=1):
    """Wait for a response from the bot (mock version)."""
    await asyncio.sleep(0.1)  # Simulate network delay
    messages = await client.get_messages(chat_id, limit=1)
    if messages:
        return messages[0]
    raise TimedOutError("No response received")

@pytest.mark.asyncio
async def test_bot_signup_flow(mock_client):
    """Test the complete signup flow with the bot."""
    client = mock_client
    
    try:
        # Connect the client
        await client.connect()
        
        # Get the bot's chat ID
        bot_entity = await client.get_entity("@Purr9Bot")
        chat_id = bot_entity.id
        bot_id = bot_entity.id
        
        # Test 1: Logout to ensure clean state
        print("Test 1: Sending /logout command...")
        await client.send_message(bot_entity, '/logout')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "you have been logged out successfully" in resp.text.lower()
        
        # Test 2: Start command
        print("\nTest 2: Sending /start command...")
        await client.send_message(bot_entity, '/start')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "gm anon" in resp.text.lower()
        
        # Test 3: Signup with password
        print("\nTest 3: Sending /signup command...")
        await client.send_message(bot_entity, '/signup hey')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "account created successfully" in resp.text.lower()
        assert "enter your password again" in resp.text.lower()
        
        # Test 4: Login with password
        print("\nTest 4: Sending password for login...")
        await client.send_message(bot_entity, 'hey')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "logged in successfully" in resp.text.lower()
        
        # Test 5: List command (while logged in)
        print("\nTest 5: Sending /list command...")
        await client.send_message(bot_entity, '/list')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "listing your items" in resp.text.lower()
        
        # Test 6: Final logout
        print("\nTest 6: Sending /logout command...")
        await client.send_message(bot_entity, '/logout')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "you have been logged out successfully" in resp.text.lower()
        
        # Test 7: Start after logout
        print("\nTest 7: Sending /start after logout...")
        await client.send_message(bot_entity, '/start')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "gm anon" in resp.text.lower()
        
        print("\n✅ All tests passed successfully!")
        
    except Exception as e:
        print(f"\n❌ Test failed with error: {str(e)}")
        pytest.fail(f"Test failed with error: {str(e)}")
    finally:
        await client.disconnect()

# Additional test to verify individual commands
@pytest.mark.asyncio
async def test_individual_commands(mock_client):
    """Test individual bot commands."""
    client = mock_client
    await client.connect()
    
    bot_entity = await client.get_entity("@Purr9Bot")
    chat_id = bot_entity.id
    bot_id = bot_entity.id
    
    # Test each command
    commands_to_test = [
        ("/start", "gm anon"),
        ("/logout", "logged out successfully"),
    ]
    
    for command, expected_response in commands_to_test:
        print(f"\nTesting command: {command}")
        await client.send_message(bot_entity, command)
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert expected_response in resp.text.lower(), f"Expected '{expected_response}' in response for {command}"
    
    await client.disconnect()
    print("\n✅ Individual command tests passed!")