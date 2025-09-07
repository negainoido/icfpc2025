#!/usr/bin/env python3
"""
api-serverのselectエンドポイントに対する同時リクエストのテストスクリプト
修正後は、複数のリクエストが同時に来ても1つだけが成功し、
他は409 Conflictエラーを返すことを確認する
"""

import asyncio
import aiohttp
import json
import sys
from typing import Dict, Any

API_BASE_URL = "http://localhost:3000"

async def send_select_request(session: aiohttp.ClientSession, request_id: int) -> Dict[str, Any]:
    """selectリクエストを送信し、結果を返す"""
    url = f"{API_BASE_URL}/api/select"
    payload = {
        "problemName": "test-problem",
        "user_name": f"user_{request_id}"
    }
    
    try:
        async with session.post(url, json=payload) as response:
            response_data = await response.json()
            return {
                "request_id": request_id,
                "status": response.status,
                "response": response_data,
                "success": response.status == 200
            }
    except Exception as e:
        return {
            "request_id": request_id,
            "status": -1,
            "response": {"error": str(e)},
            "success": False
        }

async def test_concurrent_select(num_requests: int = 5):
    """複数の同時selectリクエストをテストする"""
    print(f"Testing {num_requests} concurrent select requests...")
    
    async with aiohttp.ClientSession() as session:
        # 複数のリクエストを並行実行
        tasks = [
            send_select_request(session, i) 
            for i in range(num_requests)
        ]
        
        results = await asyncio.gather(*tasks)
        
        # 結果を分析
        successful_requests = [r for r in results if r["success"]]
        failed_requests = [r for r in results if not r["success"]]
        conflict_requests = [r for r in results if r["status"] == 409]
        
        print("\n=== TEST RESULTS ===")
        print(f"Total requests: {len(results)}")
        print(f"Successful requests: {len(successful_requests)}")
        print(f"Failed requests: {len(failed_requests)}")
        print(f"Conflict (409) responses: {len(conflict_requests)}")
        
        print("\nSuccessful requests:")
        for r in successful_requests:
            print(f"  Request {r['request_id']}: {r['response']}")
        
        print("\nFailed requests:")
        for r in failed_requests:
            status = r['status']
            error = r['response'].get('message', 'Unknown error')
            print(f"  Request {r['request_id']}: Status {status} - {error}")
        
        # テスト判定
        if len(successful_requests) == 1 and len(conflict_requests) >= 1:
            print("\n✅ TEST PASSED: Exactly 1 request succeeded, others got 409 Conflict")
            return True
        else:
            print("\n❌ TEST FAILED: Expected exactly 1 success and at least 1 conflict")
            return False

async def cleanup_sessions():
    """テスト後のクリーンアップ: 全セッションを取得してアクティブなセッションがあれば中断"""
    print("Cleaning up active sessions...")
    
    async with aiohttp.ClientSession() as session:
        # 現在のアクティブセッションを取得
        try:
            async with session.get(f"{API_BASE_URL}/api/sessions/current") as response:
                if response.status == 200:
                    current_session = await response.json()
                    if current_session:  # セッションが存在する場合
                        session_id = current_session["session_id"]
                        print(f"Found active session: {session_id}")
                        
                        # セッションを中断
                        async with session.put(f"{API_BASE_URL}/api/sessions/{session_id}/abort") as abort_response:
                            if abort_response.status == 200:
                                print(f"Successfully aborted session: {session_id}")
                            else:
                                print(f"Failed to abort session: {abort_response.status}")
                    else:
                        print("No active session found")
        except Exception as e:
            print(f"Cleanup error: {e}")

async def main():
    if len(sys.argv) > 1:
        try:
            num_requests = int(sys.argv[1])
        except ValueError:
            print("Usage: python test_concurrent_select.py [number_of_requests]")
            sys.exit(1)
    else:
        num_requests = 5
    
    # クリーンアップ
    await cleanup_sessions()
    
    # テスト実行
    success = await test_concurrent_select(num_requests)
    
    # 再度クリーンアップ
    await cleanup_sessions()
    
    sys.exit(0 if success else 1)

if __name__ == "__main__":
    asyncio.run(main())