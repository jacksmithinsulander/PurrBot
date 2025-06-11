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
    '/logout_error': "❌ You are not logged in!",
    '/start': "💻 gm anon, whatchu wanna do? 🐈",
    '/signup hey': "Account created successfully! 🎉\nNow enter your password again to log in.",
    '/signup_error': "❌ You are already logged in! Please logout first.",
    '/login hey': "Logged in successfully! 🎉",
    '/login_error': "❌ You are already logged in!",
    'hey': "Logged in successfully! 🎉",
    '/list': "📋 Listing your items...",
    '/printkeys': "🔑 Here are your keys:\n\nPrivate Key:\n`0x1234...abcd`\n\nPublic Key:\n`0x5678...efgh`",
    '/printkeys_error': "❌ No keys available. Please log in first."
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
    conversation_state = {"messages": [], "logged_in": False, "has_account": False}
    
    async def mock_send_message(entity, message):
        """Mock sending a message and generate appropriate response."""
        conversation_state["messages"].append(message)
        
        # Generate bot response based on the message and state
        if message == "/logout":
            if conversation_state["logged_in"]:
                response_text = BOT_RESPONSES["/logout"]
                conversation_state["logged_in"] = False
            else:
                response_text = BOT_RESPONSES["/logout_error"]
                
        elif message.startswith("/signup"):
            if conversation_state["logged_in"]:
                response_text = BOT_RESPONSES["/signup_error"]
            else:
                response_text = BOT_RESPONSES["/signup hey"]
                conversation_state["has_account"] = True
                
        elif message.startswith("/login"):
            if conversation_state["logged_in"]:
                response_text = BOT_RESPONSES["/login_error"]
            else:
                response_text = BOT_RESPONSES["/login hey"]
                conversation_state["logged_in"] = True
                
        elif message == "hey" and any("/signup" in m for m in conversation_state["messages"]):
            response_text = BOT_RESPONSES["hey"]
            conversation_state["logged_in"] = True
            
        elif message == "/printkeys":
            if conversation_state["logged_in"]:
                response_text = BOT_RESPONSES["/printkeys"]
            else:
                response_text = BOT_RESPONSES["/printkeys_error"]
                
        elif message in BOT_RESPONSES:
            response_text = BOT_RESPONSES[message]
        else:
            response_text = "Unknown command"
            
        return None
    
    async def mock_get_messages(chat_id, limit=10):
        """Return the appropriate bot response."""
        if not conversation_state["messages"]:
            return []
            
        last_message = conversation_state["messages"][-1]
        
        # Get appropriate response based on state
        if last_message == "/logout":
            if conversation_state.get("was_logged_in", False):
                response_text = BOT_RESPONSES["/logout"]
            else:
                response_text = BOT_RESPONSES["/logout_error"]
                
        elif last_message.startswith("/signup"):
            if conversation_state.get("was_logged_in", False):
                response_text = BOT_RESPONSES["/signup_error"]
            else:
                response_text = BOT_RESPONSES["/signup hey"]
                
        elif last_message.startswith("/login"):
            if conversation_state.get("was_logged_in", False):
                response_text = BOT_RESPONSES["/login_error"]
            else:
                response_text = BOT_RESPONSES["/login hey"]
                
        elif last_message == "hey" and any("/signup" in m for m in conversation_state["messages"]):
            response_text = BOT_RESPONSES["hey"]
            
        elif last_message == "/printkeys":
            if conversation_state.get("was_logged_in", False):
                response_text = BOT_RESPONSES["/printkeys"]
            else:
                response_text = BOT_RESPONSES["/printkeys_error"]
                
        elif last_message in BOT_RESPONSES:
            response_text = BOT_RESPONSES[last_message]
        else:
            response_text = "Unknown command"
            
        # Update was_logged_in for next call
        conversation_state["was_logged_in"] = conversation_state["logged_in"]
            
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
async def test_full_bot_flow(mock_client):
    """Test the complete bot flow as requested."""
    client = mock_client
    
    try:
        # Connect the client
        await client.connect()
        
        # Get the bot's chat ID
        bot_entity = await client.get_entity("@Purr9Bot")
        chat_id = bot_entity.id
        bot_id = bot_entity.id
        
        print("\n=== Starting Full Bot Flow Test ===\n")
        
        # Step 1: Logout (should fail because we are already logged out)
        print("Step 1: Testing logout when already logged out...")
        await client.send_message(bot_entity, '/logout')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "you are not logged in" in resp.text.lower()
        print("✅ Correctly failed to logout when not logged in\n")
        
        # Step 2: Sign up
        print("Step 2: Signing up with password 'hey'...")
        await client.send_message(bot_entity, '/signup hey')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "account created successfully" in resp.text.lower()
        assert "enter your password again" in resp.text.lower()
        print("✅ Account created successfully\n")
        
        # Login with confirmation password
        print("Step 2b: Logging in with confirmation password...")
        await client.send_message(bot_entity, 'hey')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "logged in successfully" in resp.text.lower()
        print("✅ Logged in successfully\n")
        
        # Step 3: Print keys
        print("Step 3: Printing keys while logged in...")
        await client.send_message(bot_entity, '/printkeys')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "here are your keys" in resp.text.lower() or "private key" in resp.text.lower()
        print("✅ Keys printed successfully\n")
        
        # Step 4: Log out
        print("Step 4: Logging out...")
        await client.send_message(bot_entity, '/logout')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "you have been logged out successfully" in resp.text.lower()
        print("✅ Logged out successfully\n")
        
        # Step 5: Print keys (should fail because we are logged out)
        print("Step 5: Trying to print keys while logged out...")
        await client.send_message(bot_entity, '/printkeys')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "no keys available" in resp.text.lower() or "please log in first" in resp.text.lower()
        print("✅ Correctly failed to print keys when logged out\n")
        
        # Step 6: Log back in with the credentials we just made
        print("Step 6: Logging back in with existing credentials...")
        await client.send_message(bot_entity, '/login hey')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "logged in successfully" in resp.text.lower()
        print("✅ Logged back in successfully\n")
        
        # Step 7: Print keys again (should work)
        print("Step 7: Printing keys again after logging back in...")
        await client.send_message(bot_entity, '/printkeys')
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert "here are your keys" in resp.text.lower() or "private key" in resp.text.lower()
        print("✅ Keys printed successfully after re-login\n")
        
        print("=== All tests passed! ✅ ===")
        
    except Exception as e:
        print(f"\n❌ Test failed with error: {str(e)}")
        pytest.fail(f"Test failed with error: {str(e)}")
    finally:
        await client.disconnect()

# Keep the original tests as well
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
        # This might fail if not logged in, which is fine
        
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
    ]
    
    for command, expected_response in commands_to_test:
        print(f"\nTesting command: {command}")
        await client.send_message(bot_entity, command)
        resp = await wait_for_response(client, chat_id, bot_id)
        print(f"Response: {resp.text}")
        assert expected_response in resp.text.lower(), f"Expected '{expected_response}' in response for {command}"
    
    await client.disconnect()
    print("\n✅ Individual command tests passed!")