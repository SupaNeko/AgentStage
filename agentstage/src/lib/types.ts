export interface Agent {
    id: string;
    name: string;
    avatar_path: string | null;
    detailed_persona: string;
    simplified_persona: string;
    model_provider: string | null;
    model_name: string | null;
    created_at: number;
}
