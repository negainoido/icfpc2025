#!/usr/bin/env python3
"""
Test to understand when the solver creates more rooms than expected.
"""

def simulate_exploration():
    """
    Simulate how the solver explores and creates rooms.
    """
    print("Simulating exploration of a graph where multiple paths lead to same vertices...")
    
    # Example: A simple graph with 3 vertices
    # Vertex 0 (label A): door 0 -> vertex 1, door 1 -> vertex 2
    # Vertex 1 (label B): door 0 -> vertex 0, door 1 -> vertex 2  
    # Vertex 2 (label A): door 0 -> vertex 0, door 1 -> vertex 1
    
    # Notice vertices 0 and 2 have the same label A
    
    # Solver starts at vertex 0
    rooms = {0: {"label": "A", "path": ""}}
    frontier = [0]
    room_counter = 1
    
    print("\nStarting exploration from room 0 (label A, path '')")
    
    # Explore room 0
    print("\nExploring room 0:")
    print("  Door 0 -> label B (path '0')")
    print("  Door 1 -> label A (path '1')")
    
    # Door 0 leads to label B (new)
    print("  Creating room 1 with label B at path '0'")
    rooms[1] = {"label": "B", "path": "0"}
    frontier.append(1)
    room_counter += 1
    
    # Door 1 leads to label A (same as room 0)
    print("  Door 1 has label A, checking equivalence with room 0...")
    print("  - Room at path '1' vs room 0 at path ''")
    print("  - If these are different vertices (even with same label), they're not equivalent!")
    print("  - Creating room 2 with label A at path '1'")
    rooms[2] = {"label": "A", "path": "1"}
    frontier.append(2)
    room_counter += 1
    
    print(f"\nResult: Created {len(rooms)} rooms, but actual graph has only 3 vertices")
    print("This happens because paths '1' and '' lead to different vertices with same label")
    
simulate_exploration()