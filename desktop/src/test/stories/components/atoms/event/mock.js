import mimo from "../../../assets/mimo.png";

export const EVENT_CREATOR = {
    title: "My event",
    is_creator: true,
    description: "My event description !",
    start_time: "2019/05/15",
    location: "Paris",
    participants: [
        {
            avatar_url: mimo,
            username: "Jon",
            status: "accepted"
        }
    ]
}

export const EVENT_PARTICIPANT_ACCEPTED = {
    title: "My event",
    is_creator: false,
    description: "My event description !",
    start_time: "2019/05/15",
    location: "Paris",
    participants: [
        {
            avatar_url: mimo,
            username: "Jon",
            status: "accepted"
        }
    ],
    my_status: "accepted"
}

export const EVENT_PARTICIPANTS_5 = {
    title: "My event",
    is_creator: false,
    description: "My event description !",
    start_time: "2019/05/15",
    location: "Paris",
    participants: [
        {
            avatar_url: mimo,
            username: "Jon",
            status: "accepted"
        },
        {
            avatar_url: mimo,
            username: "Doe",
            status: "declined"
        },
        {
            avatar_url: mimo,
            username: "Bob",
            status: "maybe"
        },
        {
            avatar_url: mimo,
            username: "Alice",
            status: "maybe"
        },
        {
            avatar_url: mimo,
            username: "Charles",
            status: "accepted"
        }
    ],
    my_status: "accepted"
}

export const EVENT_PARTICIPANTS_6 = {
    ...EVENT_PARTICIPANTS_5,
    participants: [...EVENT_PARTICIPANTS_5.participants, 
        {
            avatar_url: mimo,
            username: "Kim",
            status: "declined"
        }
    ]
}